use signal_auras_core::{
    ActiveProcessContext, Capability, CapabilityKind, CapabilityReport, CapabilitySet,
    CleanupReport, DiagnosableError, ErrorPhase, InputEmission, MacroAction,
    SynthesizedInputRequest,
};

use crate::{capability::environment_probe, diagnostics::unsupported_protocol};

use ashpd::desktop::{
    remote_desktop::{
        DeviceType, KeyState, NotifyKeyboardKeysymOptions, NotifyPointerButtonOptions,
        RemoteDesktop, SelectDevicesOptions,
    },
    Session,
};

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
        MacroAction::KeyPress { key } => {
            let keysym = key_name_to_keysym(key).ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    format!("key '{key}' is unsupported by the KDE portal key translation path"),
                )
                .with_capability(Capability::SynthesizedInput)
                .with_source("xdg-desktop-portal RemoteDesktop")
                .with_remediation("use a supported named key or ASCII text input")
            })?;
            emit_keysym(proxy, session, keysym)
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
