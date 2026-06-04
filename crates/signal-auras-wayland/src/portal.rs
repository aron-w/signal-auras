use signal_auras_core::{
    ActiveProcessContext, Capability, CapabilityKind, CapabilityReport, CapabilitySet,
    CleanupReport, DiagnosableError, ErrorPhase, InputEmission, MacroAction, ScreenPixelFormat,
    ScreenSample, SynthesizedInputRequest,
};
use std::{
    cell::RefCell,
    env, fs,
    os::fd::OwnedFd,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{capability::environment_probe, diagnostics::unsupported_protocol};

use ashpd::desktop::{
    remote_desktop::{
        DeviceType, KeyState, NotifyKeyboardKeysymOptions, NotifyPointerButtonOptions,
        RemoteDesktop, SelectDevicesOptions,
    },
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream},
    PersistMode, Session,
};
use pipewire as pw;
use pw::{properties::properties, spa};

pub fn probe_required_capabilities(required: &CapabilitySet) -> CapabilityReport {
    environment_probe(required)
}

pub fn probe_global_shortcuts() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::GlobalShortcut))
}

pub fn probe_synthesized_input() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::SynthesizedInput))
}

pub fn synthesized_input_capability_set() -> CapabilitySet {
    CapabilitySet::new([CapabilityKind::SynthesizedInput])
}

pub fn active_process_context() -> Result<ActiveProcessContext, DiagnosableError> {
    Ok(ActiveProcessContext::unavailable(
        "active process metadata provider is unsupported",
    ))
}

pub fn synthesize_input(
    request: SynthesizedInputRequest,
) -> Result<InputEmission, DiagnosableError> {
    crate::input::validate_request_for_portal(&request)?;
    Ok(InputEmission::Emitted)
}

#[derive(Debug)]
enum PortalInputBackend {
    Validating,
    RemoteDesktop {
        proxy: RemoteDesktop,
        session: Session<RemoteDesktop>,
    },
}

#[derive(Debug)]
pub struct PortalInputSession {
    active: bool,
    backend: PortalInputBackend,
}

impl PortalInputSession {
    pub fn open() -> Self {
        Self {
            active: true,
            backend: PortalInputBackend::Validating,
        }
    }

    pub fn open_live() -> Result<Self, DiagnosableError> {
        let (proxy, session) = portal_block_on(async {
            let proxy = RemoteDesktop::new().await?;
            let session = proxy.create_session(Default::default()).await?;
            proxy
                .select_devices(
                    &session,
                    SelectDevicesOptions::default()
                        .set_devices(DeviceType::Keyboard | DeviceType::Pointer),
                )
                .await?
                .response()?;
            proxy
                .start(&session, None, Default::default())
                .await?
                .response()?;
            Ok::<_, ashpd::Error>((proxy, session))
        })
        .map_err(portal_error)?;

        Ok(Self {
            active: true,
            backend: PortalInputBackend::RemoteDesktop { proxy, session },
        })
    }

    pub fn synthesize(
        &self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        if !self.active {
            return Ok(InputEmission::Cancelled);
        }
        crate::input::validate_request_for_portal(&request)?;
        match &self.backend {
            PortalInputBackend::Validating => Ok(InputEmission::Emitted),
            PortalInputBackend::RemoteDesktop { proxy, session } => {
                emit_request(proxy, session, &request)?;
                Ok(InputEmission::Emitted)
            }
        }
    }

    pub fn close(&mut self) -> CleanupReport {
        if self.active {
            if let PortalInputBackend::RemoteDesktop { session, .. } = &self.backend {
                let _ = portal_block_on(session.close());
            }
            self.active = false;
            CleanupReport::all_succeeded(1)
        } else {
            CleanupReport::empty()
        }
    }
}

#[derive(Debug)]
struct ScreenCastPortalHandles {
    session: Session<Screencast>,
    stream: Stream,
    pipewire_fd: OwnedFd,
}

#[derive(Debug)]
struct ScreenCastFrameState {
    latest: Option<ScreenSample>,
    last_error: Option<String>,
}

#[derive(Debug)]
struct ScreenCastPipeWireUserData {
    format: spa::param::video::VideoInfoRaw,
    latest: Rc<RefCell<ScreenCastFrameState>>,
    started_at: Instant,
}

pub struct PortalScreenCastSession {
    active: bool,
    portal_session: Session<Screencast>,
    main_loop: pw::main_loop::MainLoopRc,
    _context: pw::context::ContextRc,
    _core: pw::core::CoreRc,
    stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<ScreenCastPipeWireUserData>,
    latest: Rc<RefCell<ScreenCastFrameState>>,
}

impl PortalScreenCastSession {
    pub fn open_live() -> Result<Self, DiagnosableError> {
        tracing::trace!(
            event = "screen_read_session",
            phase = "portal_begin",
            "creating xdg-desktop-portal ScreenCast session"
        );
        let handles = portal_block_on(open_screencast_portal()).map_err(screen_cast_error)?;
        tracing::trace!(
            event = "screen_read_session",
            phase = "pipewire_begin",
            node_id = handles.stream.pipe_wire_node_id(),
            stream_size = ?handles.stream.size(),
            stream_source = ?handles.stream.source_type(),
            "connecting PipeWire stream"
        );
        open_pipewire_screencast(handles).map_err(pipewire_screen_cast_error)
    }

    pub fn capture_latest(&mut self) -> Result<ScreenSample, DiagnosableError> {
        self.latest.borrow_mut().latest = None;
        let deadline = Instant::now() + Duration::from_millis(500);
        tracing::trace!(
            event = "screen_read_capture",
            phase = "wait_frame",
            timeout_ms = 500u64,
            "waiting for readable PipeWire frame"
        );
        while Instant::now() < deadline {
            self.main_loop
                .loop_()
                .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(25)));
            let mut latest = self.latest.borrow_mut();
            if let Some(sample) = latest.latest.take() {
                tracing::trace!(
                    event = "screen_read_capture",
                    phase = "frame_ready",
                    width = sample.width,
                    height = sample.height,
                    stride = sample.stride,
                    pixel_format = ?sample.pixel_format,
                    byte_len = sample.pixels.len(),
                    captured_at_ms = sample.captured_at_ms,
                    "readable screen frame captured"
                );
                return Ok(sample);
            }
            if let Some(message) = latest.last_error.take() {
                tracing::trace!(
                    event = "screen_read_capture",
                    phase = "frame_error",
                    reason = %message,
                    "PipeWire frame was not readable"
                );
                return Err(screen_read_error(
                    message,
                    "pipewire",
                    "use a screen source that offers CPU-readable RGB/RGBA/BGRx buffers",
                ));
            }
        }
        tracing::trace!(
            event = "screen_read_capture",
            phase = "timeout",
            "timed out waiting for readable PipeWire frame"
        );
        Err(screen_read_error(
            "screen_read capture timed out before a readable PipeWire frame arrived",
            "pipewire",
            "grant ScreenCast permission and select a visible monitor or window",
        ))
    }

    pub fn close(&mut self) -> CleanupReport {
        if !self.active {
            return CleanupReport::empty();
        }
        let _ = self.stream.disconnect();
        let _ = portal_block_on(self.portal_session.close());
        self.active = false;
        tracing::trace!(
            event = "screen_read_session",
            phase = "closed",
            "closed xdg-desktop-portal ScreenCast session"
        );
        CleanupReport::all_succeeded(1)
    }
}

impl Drop for PortalScreenCastSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

async fn open_screencast_portal() -> ashpd::Result<ScreenCastPortalHandles> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session(Default::default()).await?;
    let restore_token = read_screen_cast_restore_token();
    let mut select_options = SelectSourcesOptions::default()
        .set_cursor_mode(CursorMode::Hidden)
        .set_sources(SourceType::Monitor | SourceType::Window)
        .set_multiple(false)
        .set_persist_mode(PersistMode::ExplicitlyRevoked);
    if let Some(token) = restore_token.as_deref() {
        select_options = select_options.set_restore_token(Some(token));
    }
    proxy
        .select_sources(&session, select_options)
        .await?
        .response()?;

    let response = proxy
        .start(&session, None, Default::default())
        .await?
        .response()?;
    if let Some(token) = response.restore_token() {
        write_screen_cast_restore_token(token);
    }
    let stream = response.streams().first().cloned().ok_or_else(|| {
        ashpd::Error::Portal(ashpd::PortalError::Failed(
            "no ScreenCast stream selected".into(),
        ))
    })?;
    tracing::trace!(
        event = "screen_read_session",
        phase = "portal_started",
        node_id = stream.pipe_wire_node_id(),
        stream_size = ?stream.size(),
        stream_position = ?stream.position(),
        stream_source = ?stream.source_type(),
        "portal ScreenCast stream selected"
    );
    let pipewire_fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await?;
    Ok(ScreenCastPortalHandles {
        session,
        stream,
        pipewire_fd,
    })
}

fn read_screen_cast_restore_token() -> Option<String> {
    let path = screen_cast_restore_token_path()?;
    match fs::read_to_string(&path) {
        Ok(token) => not_empty(token.trim()).map(str::to_string),
        Err(error) => {
            tracing::debug!(
                event = "screen_read_restore_token",
                phase = "read",
                path = %path.display(),
                error = %error,
                "no reusable xdg-desktop-portal ScreenCast restore token loaded"
            );
            None
        }
    }
}

fn write_screen_cast_restore_token(token: &str) {
    let Some(token) = not_empty(token.trim()) else {
        return;
    };
    let Some(path) = screen_cast_restore_token_path() else {
        tracing::debug!(
            event = "screen_read_restore_token",
            phase = "write",
            "no user state directory available for xdg-desktop-portal ScreenCast restore token"
        );
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            tracing::warn!(
                event = "screen_read_restore_token",
                phase = "write",
                path = %path.display(),
                error = %error,
                "failed to create directory for xdg-desktop-portal ScreenCast restore token"
            );
            return;
        }
    }
    if let Err(error) = fs::write(&path, token) {
        tracing::warn!(
            event = "screen_read_restore_token",
            phase = "write",
            path = %path.display(),
            error = %error,
            "failed to store xdg-desktop-portal ScreenCast restore token"
        );
        return;
    }
    tracing::debug!(
        event = "screen_read_restore_token",
        phase = "write",
        path = %path.display(),
        "stored xdg-desktop-portal ScreenCast restore token"
    );
}

fn screen_cast_restore_token_path() -> Option<PathBuf> {
    screen_cast_restore_token_path_from_env(env::var_os("XDG_STATE_HOME"), env::var_os("HOME"))
}

fn screen_cast_restore_token_path_from_env(
    xdg_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let base = xdg_state_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(base.join("signal-auras").join("screencast-restore-token"))
}

fn not_empty(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn open_pipewire_screencast(
    handles: ScreenCastPortalHandles,
) -> Result<PortalScreenCastSession, pw::Error> {
    pw::init();
    let main_loop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&main_loop, None)?;
    let core = context.connect_fd_rc(handles.pipewire_fd, None)?;
    let latest = Rc::new(RefCell::new(ScreenCastFrameState {
        latest: None,
        last_error: None,
    }));
    let stream = pw::stream::StreamRc::new(
        core.clone(),
        "signal-auras-screen-read",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;
    let user_data = ScreenCastPipeWireUserData {
        format: Default::default(),
        latest: latest.clone(),
        started_at: Instant::now(),
    };
    let listener = stream
        .add_local_listener_with_user_data(user_data)
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Ok((media_type, media_subtype)) = spa::param::format_utils::parse_format(param)
            else {
                return;
            };
            if media_type != spa::param::format::MediaType::Video
                || media_subtype != spa::param::format::MediaSubtype::Raw
            {
                return;
            }
            if let Err(error) = user_data.format.parse(param) {
                user_data.latest.borrow_mut().last_error =
                    Some(format!("cannot parse PipeWire video format: {error:?}"));
                tracing::trace!(
                    event = "screen_read_pipewire_format",
                    phase = "parse_error",
                    error = ?error,
                    "failed to parse PipeWire video format"
                );
                return;
            }
            let size = user_data.format.size();
            tracing::trace!(
                event = "screen_read_pipewire_format",
                phase = "negotiated",
                width = size.width,
                height = size.height,
                format = ?user_data.format.format(),
                framerate_num = user_data.format.framerate().num,
                framerate_denom = user_data.format.framerate().denom,
                "PipeWire video format negotiated"
            );
        })
        .process(|stream, user_data| {
            let sample = copy_pipewire_frame(stream, user_data);
            let mut latest = user_data.latest.borrow_mut();
            match sample {
                Ok(sample) => latest.latest = Some(sample),
                Err(message) => latest.last_error = Some(message),
            }
        })
        .register()?;
    let params = screen_cast_format_param();
    let mut param_refs = [spa::pod::Pod::from_bytes(&params).ok_or(pw::Error::CreationFailed)?];
    stream.connect(
        spa::utils::Direction::Input,
        Some(handles.stream.pipe_wire_node_id()),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut param_refs,
    )?;

    Ok(PortalScreenCastSession {
        active: true,
        portal_session: handles.session,
        main_loop,
        _context: context,
        _core: core,
        stream,
        _listener: listener,
        latest,
    })
}

fn screen_cast_format_param() -> Vec<u8> {
    let object = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            spa::param::video::VideoFormat::BGRx,
            spa::param::video::VideoFormat::BGRx,
            spa::param::video::VideoFormat::RGBx,
            spa::param::video::VideoFormat::BGRA,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::BGR,
            spa::param::video::VideoFormat::RGB,
            spa::param::video::VideoFormat::GRAY8,
        )
    );
    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .expect("screen cast format parameter is serializable")
    .0
    .into_inner()
}

fn copy_pipewire_frame(
    stream: &pw::stream::Stream,
    user_data: &ScreenCastPipeWireUserData,
) -> Result<ScreenSample, String> {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return Err("PipeWire stream had no buffer to dequeue".to_string());
    };
    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else {
        return Err("PipeWire buffer did not contain image data".to_string());
    };
    let buffer_type = data.type_();
    if !matches!(
        buffer_type,
        spa::buffer::DataType::MemPtr | spa::buffer::DataType::MemFd
    ) {
        return Err(format!(
            "unsupported PipeWire screen_read buffer type {:?}; only mapped MemPtr/MemFd CPU memory is supported",
            buffer_type
        ));
    }
    if !data.flags().contains(spa::buffer::DataFlags::READABLE) {
        return Err("PipeWire screen_read buffer is not marked readable".to_string());
    }
    let Some(format) = screen_pixel_format(user_data.format.format()) else {
        return Err(format!(
            "unsupported PipeWire video format {:?}",
            user_data.format.format()
        ));
    };
    let size = user_data.format.size();
    let width = size.width;
    let height = size.height;
    if width == 0 || height == 0 {
        return Err("PipeWire video format has empty dimensions".to_string());
    }
    let chunk = data.chunk();
    if chunk.stride() <= 0 {
        return Err("PipeWire screen_read buffer uses unsupported negative stride".to_string());
    }
    let stride = u32::try_from(chunk.stride()).map_err(|_| {
        "PipeWire screen_read buffer stride does not fit the screen sample model".to_string()
    })?;
    let bytes_per_pixel = format.bytes_per_pixel() as u32;
    if stride < width.saturating_mul(bytes_per_pixel) {
        return Err("PipeWire screen_read buffer stride is smaller than one image row".to_string());
    }
    let offset = chunk.offset() as usize;
    let required = height as usize * stride as usize;
    let Some(mapped) = data.data() else {
        return Err("PipeWire screen_read buffer was not mapped into CPU memory".to_string());
    };
    let end = offset
        .checked_add(required)
        .ok_or_else(|| "PipeWire screen_read buffer size overflow".to_string())?;
    if end > mapped.len() {
        return Err("PipeWire screen_read buffer is shorter than the advertised frame".to_string());
    }
    tracing::trace!(
        event = "screen_read_pipewire_frame",
        width,
        height,
        stride,
        pixel_format = ?format,
        byte_len = required,
        buffer_type = ?buffer_type,
        "copied mapped PipeWire frame into owned screen sample"
    );
    Ok(ScreenSample::from_pixels(
        width,
        height,
        stride,
        format,
        user_data.started_at.elapsed().as_millis() as u64,
        mapped[offset..end].to_vec(),
    ))
}

fn screen_pixel_format(format: spa::param::video::VideoFormat) -> Option<ScreenPixelFormat> {
    match format {
        value if value == spa::param::video::VideoFormat::GRAY8 => Some(ScreenPixelFormat::Luma8),
        value if value == spa::param::video::VideoFormat::RGB => Some(ScreenPixelFormat::Rgb888),
        value if value == spa::param::video::VideoFormat::BGR => Some(ScreenPixelFormat::Bgr888),
        value if value == spa::param::video::VideoFormat::RGBA => Some(ScreenPixelFormat::Rgba8888),
        value if value == spa::param::video::VideoFormat::BGRA => Some(ScreenPixelFormat::Bgra8888),
        value if value == spa::param::video::VideoFormat::RGBx => Some(ScreenPixelFormat::Rgbx8888),
        value if value == spa::param::video::VideoFormat::BGRx => Some(ScreenPixelFormat::Bgrx8888),
        _ => None,
    }
}

fn emit_request(
    proxy: &RemoteDesktop,
    session: &Session<RemoteDesktop>,
    request: &SynthesizedInputRequest,
) -> Result<(), DiagnosableError> {
    match &request.action {
        MacroAction::TextInput { text } => {
            for keysym in text.chars().map(text_char_to_keysym) {
                emit_keysym(proxy, session, keysym)?;
            }
            Ok(())
        }
        MacroAction::KeyPress { key }
        | MacroAction::KeyDown { key }
        | MacroAction::KeyUp { key } => {
            let keysym = key_name_to_keysym(key).ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    format!("key '{key}' is unsupported by the KDE portal key translation path"),
                )
                .with_capability(Capability::SynthesizedInput)
                .with_source("xdg-desktop-portal RemoteDesktop")
                .with_remediation("use a supported named key or ASCII text input")
            })?;
            match &request.action {
                MacroAction::KeyDown { .. } => {
                    emit_keysym_state(proxy, session, keysym, KeyState::Pressed)
                }
                MacroAction::KeyUp { .. } => {
                    emit_keysym_state(proxy, session, keysym, KeyState::Released)
                }
                _ => emit_keysym(proxy, session, keysym),
            }
        }
        MacroAction::MouseClick { button } => {
            let evdev_button = mouse_button_to_evdev(*button);
            emit_pointer_button(proxy, session, evdev_button, KeyState::Pressed)?;
            emit_pointer_button(proxy, session, evdev_button, KeyState::Released)
        }
        MacroAction::Delay { .. } => Ok(()),
    }
}

fn emit_keysym(
    proxy: &RemoteDesktop,
    session: &Session<RemoteDesktop>,
    keysym: i32,
) -> Result<(), DiagnosableError> {
    portal_block_on(async {
        proxy
            .notify_keyboard_keysym(
                session,
                keysym,
                KeyState::Pressed,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await?;
        proxy
            .notify_keyboard_keysym(
                session,
                keysym,
                KeyState::Released,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await
    })
    .map_err(portal_error)
}

fn emit_keysym_state(
    proxy: &RemoteDesktop,
    session: &Session<RemoteDesktop>,
    keysym: i32,
    state: KeyState,
) -> Result<(), DiagnosableError> {
    portal_block_on(async {
        proxy
            .notify_keyboard_keysym(
                session,
                keysym,
                state,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await
    })
    .map_err(portal_error)
}

fn emit_pointer_button(
    proxy: &RemoteDesktop,
    session: &Session<RemoteDesktop>,
    button: i32,
    state: KeyState,
) -> Result<(), DiagnosableError> {
    portal_block_on(async {
        proxy
            .notify_pointer_button(
                session,
                button,
                state,
                NotifyPointerButtonOptions::default(),
            )
            .await
    })
    .map_err(portal_error)
}

fn mouse_button_to_evdev(button: signal_auras_core::MouseButton) -> i32 {
    match button {
        signal_auras_core::MouseButton::Left => 0x110,
        signal_auras_core::MouseButton::Right => 0x111,
        signal_auras_core::MouseButton::Middle => 0x112,
    }
}

fn text_char_to_keysym(character: char) -> i32 {
    character as i32
}

fn key_name_to_keysym(key: &str) -> Option<i32> {
    match key.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(0xff0d),
        "tab" => Some(0xff09),
        "escape" | "esc" => Some(0xff1b),
        "backspace" => Some(0xff08),
        "delete" | "del" => Some(0xffff),
        "space" => Some(0x20),
        key if key.chars().count() == 1 => key.chars().next().map(text_char_to_keysym),
        _ => None,
    }
}

fn portal_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    zbus::block_on(future)
}

fn portal_error(error: ashpd::Error) -> DiagnosableError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
        | ashpd::Error::Portal(ashpd::PortalError::Cancelled(_)) => {
            crate::diagnostics::denied_permission(Capability::SynthesizedInput)
                .with_source("xdg-desktop-portal RemoteDesktop")
        }
        other => DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "KDE portal synthesized input request failed",
        )
        .with_capability(Capability::SynthesizedInput)
        .with_source(other.to_string())
        .with_remediation("grant RemoteDesktop keyboard control permission and retry"),
    }
}

fn screen_cast_error(error: ashpd::Error) -> DiagnosableError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
        | ashpd::Error::Portal(ashpd::PortalError::Cancelled(_)) => {
            crate::diagnostics::denied_permission(Capability::ScreenRead)
                .with_source("xdg-desktop-portal ScreenCast")
        }
        other => screen_read_error(
            "KDE portal ScreenCast request failed",
            other.to_string(),
            "grant ScreenCast permission and retry",
        ),
    }
}

fn pipewire_screen_cast_error(error: pw::Error) -> DiagnosableError {
    screen_read_error(
        "PipeWire screen_read stream setup failed",
        error.to_string(),
        "ensure PipeWire is running and the selected ScreenCast source is readable",
    )
}

fn screen_read_error(
    message: impl Into<String>,
    source: impl Into<String>,
    remediation: impl Into<String>,
) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::CapabilityProbe, message)
        .with_capability(Capability::ScreenRead)
        .with_source(source)
        .with_remediation(remediation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn screen_cast_restore_token_prefers_xdg_state_home() {
        let path = screen_cast_restore_token_path_from_env(
            Some(OsString::from("/tmp/signal-auras-state")),
            Some(OsString::from("/home/tester")),
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from("/tmp/signal-auras-state")
                .join("signal-auras")
                .join("screencast-restore-token")
        );
    }

    #[test]
    fn screen_cast_restore_token_falls_back_to_home_state_dir() {
        let path =
            screen_cast_restore_token_path_from_env(None, Some(OsString::from("/home/tester")))
                .unwrap();

        assert_eq!(
            path,
            PathBuf::from("/home/tester")
                .join(".local/state")
                .join("signal-auras")
                .join("screencast-restore-token")
        );
    }

    #[test]
    fn screen_cast_restore_token_path_is_unavailable_without_user_state() {
        assert!(screen_cast_restore_token_path_from_env(None, None).is_none());
    }
}
