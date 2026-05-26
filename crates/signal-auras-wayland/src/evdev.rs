use signal_auras_core::{
    DiagnosableError, ErrorPhase, InputProviderMode, MotionInputEvent, MotionInputState,
    MotionToken, MouseButton, WheelDirection,
};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read},
    os::fd::{AsRawFd, RawFd},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_SYN: u16 = 0x00;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_WHEEL: u16 = 0x08;
const KEY_F: u16 = 33;
const KEY_F1: u16 = 59;
const KEY_F10: u16 = 68;
const KEY_F11: u16 = 87;
const KEY_F12: u16 = 88;
const KEY_F13: u16 = 183;
const KEY_F24: u16 = 194;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

#[derive(Debug)]
pub struct EvdevObservationProvider {
    devices: Vec<EvdevDevice>,
    leader: Option<MotionToken>,
    grabbed: bool,
    next_device_index: usize,
    all_devices: bool,
    mode: InputProviderMode,
    last_rescan: Instant,
    rescan_interval: Duration,
    skipped_paths: BTreeSet<PathBuf>,
    udev_monitor: Option<UdevInputMonitor>,
}

impl EvdevObservationProvider {
    pub fn open(
        devices: impl IntoIterator<Item = PathBuf>,
        mode: InputProviderMode,
        leader: Option<MotionToken>,
        all_devices: bool,
    ) -> Result<Self, DiagnosableError> {
        let devices = devices
            .into_iter()
            .map(EvdevDevice::open)
            .collect::<Result<Vec<_>, _>>()?;
        if devices.is_empty() {
            return Err(evdev_error(
                ErrorPhase::Registration,
                "evdev observation requires at least one input device",
                None,
            ));
        }
        let provider = Self {
            devices,
            leader,
            grabbed: false,
            next_device_index: 0,
            all_devices,
            mode,
            last_rescan: Instant::now(),
            rescan_interval: Duration::from_secs(1),
            skipped_paths: BTreeSet::new(),
            udev_monitor: if all_devices {
                match UdevInputMonitor::open() {
                    Ok(monitor) => Some(monitor),
                    Err(error) => {
                        tracing::warn!("level=warn event=udev_monitor_unavailable error={error}");
                        None
                    }
                }
            } else {
                None
            },
        };
        Ok(provider)
    }

    pub fn next_motion_event(&mut self) -> Result<Option<MotionInputEvent>, DiagnosableError> {
        Ok(self.next_observed_motion_event()?.map(|event| event.event))
    }

    pub fn next_observed_motion_event(
        &mut self,
    ) -> Result<Option<ObservedMotionInputEvent>, DiagnosableError> {
        if self.devices.is_empty() {
            return Ok(None);
        }
        for offset in 0..self.devices.len() {
            let index = (self.next_device_index + offset) % self.devices.len();
            let device = &mut self.devices[index];
            if let Some(raw) = device.next_raw_event()? {
                let Some(mut event) = decode_raw_input_event(&raw) else {
                    continue;
                };
                let source = device.path.clone();
                self.next_device_index = (index + 1) % self.devices.len();
                if self
                    .leader
                    .as_ref()
                    .is_some_and(|leader| event.token == *leader)
                {
                    event.token = MotionToken::Leader;
                }
                return Ok(Some(ObservedMotionInputEvent {
                    event,
                    source,
                    observed_at: Instant::now(),
                }));
            }
        }
        self.next_device_index = (self.next_device_index + 1) % self.devices.len();
        Ok(None)
    }

    pub fn wait_next_observed_motion_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<ObservedMotionInputEvent>, DiagnosableError> {
        match self.wait_next_observed_motion_event_or_runtime_fd(timeout, &[])? {
            EvdevWaitOutcome::Motion(event) => Ok(Some(event)),
            EvdevWaitOutcome::RuntimeFd(_) | EvdevWaitOutcome::Timeout => Ok(None),
        }
    }

    pub fn wait_next_observed_motion_event_or_runtime_fd(
        &mut self,
        timeout: Duration,
        runtime_fds: &[RawFd],
    ) -> Result<EvdevWaitOutcome, DiagnosableError> {
        if let Some(event) = self.next_observed_motion_event()? {
            return Ok(EvdevWaitOutcome::Motion(event));
        }
        if self.devices.iter().any(|device| device.active)
            || !runtime_fds.is_empty()
            || self.udev_monitor.is_some()
        {
            match self.wait_for_readable_device(timeout, runtime_fds)? {
                EvdevReadiness::RuntimeFd(fd) => return Ok(EvdevWaitOutcome::RuntimeFd(fd)),
                EvdevReadiness::DeviceOrHotplug => {}
                EvdevReadiness::Timeout => return Ok(EvdevWaitOutcome::Timeout),
            }
        } else if !timeout.is_zero() {
            std::thread::sleep(timeout.min(Duration::from_millis(50)));
        }
        if self.udev_monitor.is_none() {
            self.rescan_all_devices_if_due()?;
        }
        Ok(match self.next_observed_motion_event()? {
            Some(event) => EvdevWaitOutcome::Motion(event),
            None => EvdevWaitOutcome::Timeout,
        })
    }

    pub fn next_observed_input_event(
        &mut self,
    ) -> Result<Option<ObservedInputEvent>, DiagnosableError> {
        if self.devices.is_empty() {
            return Ok(None);
        }
        for offset in 0..self.devices.len() {
            let index = (self.next_device_index + offset) % self.devices.len();
            let device = &mut self.devices[index];
            if let Some(raw) = device.next_raw_event()? {
                let source = device.path.clone();
                let grabbed = device.grabbed;
                let next_device_index = (index + 1) % self.devices.len();
                let event = decode_raw_input_event(&raw).map(|mut event| {
                    if self
                        .leader
                        .as_ref()
                        .is_some_and(|leader| event.token == *leader)
                    {
                        event.token = MotionToken::Leader;
                    }
                    event
                });
                self.next_device_index = next_device_index;
                return Ok(Some(ObservedInputEvent {
                    raw,
                    event,
                    source,
                    grabbed,
                    observed_at: Instant::now(),
                }));
            }
        }
        self.next_device_index = (self.next_device_index + 1) % self.devices.len();
        Ok(None)
    }

    pub fn wait_next_observed_input_event_or_runtime_fd(
        &mut self,
        timeout: Duration,
        runtime_fds: &[RawFd],
    ) -> Result<EvdevInputWaitOutcome, DiagnosableError> {
        if let Some(event) = self.next_observed_input_event()? {
            return Ok(EvdevInputWaitOutcome::Input(event));
        }
        if self.devices.iter().any(|device| device.active)
            || !runtime_fds.is_empty()
            || self.udev_monitor.is_some()
        {
            match self.wait_for_readable_device(timeout, runtime_fds)? {
                EvdevReadiness::RuntimeFd(fd) => return Ok(EvdevInputWaitOutcome::RuntimeFd(fd)),
                EvdevReadiness::DeviceOrHotplug => {}
                EvdevReadiness::Timeout => return Ok(EvdevInputWaitOutcome::Timeout),
            }
        } else if !timeout.is_zero() {
            std::thread::sleep(timeout.min(Duration::from_millis(50)));
        }
        if self.udev_monitor.is_none() {
            self.rescan_all_devices_if_due()?;
        }
        Ok(match self.next_observed_input_event()? {
            Some(event) => EvdevInputWaitOutcome::Input(event),
            None => EvdevInputWaitOutcome::Timeout,
        })
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn is_grab_capable(&self) -> bool {
        self.mode == InputProviderMode::Grab
    }

    pub fn arm_grab(&mut self) -> Result<(), DiagnosableError> {
        if self.mode != InputProviderMode::Grab || self.grabbed {
            return Ok(());
        }
        self.grab_pointer_devices()
    }

    pub fn release_grab(&mut self) {
        if !self.grabbed {
            return;
        }
        for device in &mut self.devices {
            if let Err(error) = device.set_grabbed(false) {
                tracing::warn!(
                    "level=warn event=evdev_device_release_failed path={} error={}",
                    device.path.display(),
                    error
                );
            }
        }
        self.grabbed = false;
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn active_device_count(&self) -> usize {
        self.devices.iter().filter(|device| device.active).count()
    }

    pub fn rescan_all_devices_if_due(&mut self) -> Result<(), DiagnosableError> {
        if !self.all_devices || self.last_rescan.elapsed() < self.rescan_interval {
            return Ok(());
        }
        self.last_rescan = Instant::now();
        let devices = discover_event_devices()?;
        self.rescan_devices(devices);
        Ok(())
    }

    pub fn rescan_devices(&mut self, devices: impl IntoIterator<Item = PathBuf>) {
        if !self.all_devices {
            return;
        }
        let known = self
            .devices
            .iter()
            .map(|device| device.path.clone())
            .collect::<BTreeSet<_>>();
        let mut discovered = BTreeSet::new();
        for path in devices {
            discovered.insert(path.clone());
            if known.contains(&path) {
                self.skipped_paths.remove(&path);
                continue;
            }
            match EvdevDevice::open(path.clone()) {
                Ok(mut device) => {
                    self.skipped_paths.remove(&path);
                    if self.grabbed
                        && self.mode == InputProviderMode::Grab
                        && device.pointer_capable
                    {
                        if let Err(error) = device.set_grabbed(true) {
                            tracing::warn!(
                                "level=warn event=evdev_device_grab_failed path={} error={}",
                                path.display(),
                                error
                            );
                            continue;
                        }
                    }
                    tracing::info!(
                        "level=info event=evdev_device_added path={}",
                        path.display()
                    );
                    self.devices.push(device);
                }
                Err(error) => {
                    if self.skipped_paths.insert(path.clone()) {
                        tracing::warn!(
                            "level=warn event=evdev_device_skipped path={} error={}",
                            path.display(),
                            error
                        );
                    }
                }
            }
        }
        self.skipped_paths
            .retain(|path| discovered.contains(path) && !known.contains(path));
    }

    fn wait_for_readable_device(
        &mut self,
        timeout: Duration,
        runtime_fds: &[RawFd],
    ) -> Result<EvdevReadiness, DiagnosableError> {
        #[derive(Clone, Copy)]
        enum Source {
            Device,
            Udev,
            RuntimeFd(RawFd),
        }
        let mut pollfds = self
            .devices
            .iter()
            .filter(|device| device.active)
            .map(|device| libc::pollfd {
                fd: device.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect::<Vec<_>>();
        let mut sources = self
            .devices
            .iter()
            .filter(|device| device.active)
            .map(|_| Source::Device)
            .collect::<Vec<_>>();
        if let Some(monitor) = &self.udev_monitor {
            pollfds.push(libc::pollfd {
                fd: monitor.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
            sources.push(Source::Udev);
        }
        for fd in runtime_fds {
            pollfds.push(libc::pollfd {
                fd: *fd,
                events: libc::POLLIN,
                revents: 0,
            });
            sources.push(Source::RuntimeFd(*fd));
        }
        if pollfds.is_empty() {
            return Ok(EvdevReadiness::Timeout);
        }
        let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        // Safety: poll is called with a valid pointer to the pollfd buffer for
        // the buffer length, and it does not outlive the owned file descriptors.
        let result = unsafe {
            libc::poll(
                pollfds.as_mut_ptr(),
                pollfds.len() as libc::nfds_t,
                timeout_ms,
            )
        };
        if result < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            return Ok(EvdevReadiness::Timeout);
        }
        if result < 0 {
            return Err(evdev_error(
                ErrorPhase::Trigger,
                format!(
                    "cannot poll evdev input devices: {}",
                    io::Error::last_os_error()
                ),
                None,
            ));
        }
        if result == 0 {
            return Ok(EvdevReadiness::Timeout);
        }
        let mut device_or_hotplug = false;
        let mut should_rescan = false;
        for (pollfd, source) in pollfds.iter().zip(sources) {
            if pollfd.revents & libc::POLLIN == 0 {
                continue;
            }
            match source {
                Source::Device => device_or_hotplug = true,
                Source::Udev => {
                    if let Some(monitor) = &self.udev_monitor {
                        should_rescan |= monitor.drain_has_input_event();
                    }
                }
                Source::RuntimeFd(fd) => return Ok(EvdevReadiness::RuntimeFd(fd)),
            }
        }
        if should_rescan {
            self.last_rescan = Instant::now();
            self.reconcile_discovered_devices(discover_event_devices()?);
            device_or_hotplug = true;
        }
        Ok(if device_or_hotplug {
            EvdevReadiness::DeviceOrHotplug
        } else {
            EvdevReadiness::Timeout
        })
    }

    fn grab_pointer_devices(&mut self) -> Result<(), DiagnosableError> {
        let mut grabbed = false;
        for device in &mut self.devices {
            if !device.pointer_capable {
                continue;
            }
            device.set_grabbed(true)?;
            grabbed = true;
        }
        self.grabbed = grabbed;
        Ok(())
    }

    fn reconcile_discovered_devices(&mut self, devices: impl IntoIterator<Item = PathBuf>) {
        let discovered = devices.into_iter().collect::<BTreeSet<_>>();
        for device in &mut self.devices {
            if device.active && !discovered.contains(&device.path) {
                device.active = false;
                tracing::warn!(
                    "level=warn event=evdev_device_removed path={}",
                    device.path.display()
                );
            }
        }
        self.rescan_devices(discovered);
    }
}

#[derive(Debug, Clone)]
pub enum EvdevWaitOutcome {
    Motion(ObservedMotionInputEvent),
    RuntimeFd(RawFd),
    Timeout,
}

#[derive(Debug, Clone)]
pub enum EvdevInputWaitOutcome {
    Input(ObservedInputEvent),
    RuntimeFd(RawFd),
    Timeout,
}

enum EvdevReadiness {
    DeviceOrHotplug,
    RuntimeFd(RawFd),
    Timeout,
}

#[derive(Debug, Clone)]
pub struct ObservedMotionInputEvent {
    pub event: MotionInputEvent,
    pub source: PathBuf,
    pub observed_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInputEvent {
    pub event_type: u16,
    pub code: u16,
    pub value: i32,
}

impl RawInputEvent {
    pub fn should_passthrough(&self) -> bool {
        matches!(self.event_type, EV_SYN | EV_KEY | EV_REL)
    }
}

#[derive(Debug, Clone)]
pub struct ObservedInputEvent {
    pub raw: RawInputEvent,
    pub event: Option<MotionInputEvent>,
    pub source: PathBuf,
    pub grabbed: bool,
    pub observed_at: Instant,
}

struct UdevInputMonitor {
    socket: udev::MonitorSocket,
}

impl std::fmt::Debug for UdevInputMonitor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UdevInputMonitor")
            .field("fd", &self.as_raw_fd())
            .finish()
    }
}

impl UdevInputMonitor {
    fn open() -> Result<Self, DiagnosableError> {
        let socket = udev::MonitorBuilder::new()
            .map_err(udev_error)?
            .match_subsystem("input")
            .map_err(udev_error)?
            .listen()
            .map_err(udev_error)?;
        Ok(Self { socket })
    }

    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn drain_has_input_event(&self) -> bool {
        let mut changed = false;
        for event in self.socket.iter() {
            let path = event.device().devnode().map(Path::to_path_buf);
            if path.as_deref().is_some_and(is_event_device_path) {
                tracing::info!(
                    "level=info event=udev_input_device action={} path={}",
                    event.event_type(),
                    path.as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "none".to_string())
                );
                if matches!(
                    event.event_type(),
                    udev::EventType::Add | udev::EventType::Remove | udev::EventType::Change
                ) {
                    changed = true;
                }
            }
        }
        changed
    }
}

pub fn discover_event_devices() -> Result<Vec<PathBuf>, DiagnosableError> {
    let entries = fs::read_dir("/dev/input").map_err(|error| {
        evdev_error(
            ErrorPhase::Registration,
            format!("cannot scan /dev/input for evdev devices: {error}"),
            Some(Path::new("/dev/input")),
        )
    })?;
    let mut devices = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_event_device_path(path))
        .collect::<Vec<_>>();
    devices.sort();
    if devices.is_empty() {
        return Err(evdev_error(
            ErrorPhase::Registration,
            "no /dev/input/event* devices were found",
            Some(Path::new("/dev/input")),
        ));
    }
    Ok(devices)
}

fn is_event_device_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("event"))
}

impl Drop for EvdevObservationProvider {
    fn drop(&mut self) {
        if self.grabbed {
            for device in &mut self.devices {
                let _ = device.set_grabbed(false);
            }
        }
    }
}

#[derive(Debug)]
struct EvdevDevice {
    path: PathBuf,
    file: File,
    active: bool,
    grabbed: bool,
    pointer_capable: bool,
}

impl EvdevDevice {
    fn open(path: PathBuf) -> Result<Self, DiagnosableError> {
        let file = File::open(&path).map_err(|error| {
            evdev_error(
                ErrorPhase::Registration,
                format!(
                    "cannot open evdev input device '{}': {error}",
                    path.display()
                ),
                Some(path.as_path()),
            )
        })?;
        if device_name(&file)
            .as_deref()
            .is_some_and(|name| name == crate::uinput::UINPUT_DEVICE_NAME)
        {
            return Err(evdev_error(
                ErrorPhase::Registration,
                format!(
                    "ignoring Signal Auras virtual input device '{}'",
                    path.display()
                ),
                Some(path.as_path()),
            ));
        }
        let pointer_capable = device_is_pointer_capable(&file);
        file.set_nonblocking(true).map_err(|error| {
            evdev_error(
                ErrorPhase::Registration,
                format!(
                    "cannot set evdev input device '{}' nonblocking: {error}",
                    path.display()
                ),
                Some(path.as_path()),
            )
        })?;
        Ok(Self {
            path,
            file,
            active: true,
            grabbed: false,
            pointer_capable,
        })
    }

    fn next_raw_event(&mut self) -> Result<Option<RawInputEvent>, DiagnosableError> {
        if !self.active {
            return Ok(None);
        }
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        match self.file.read_exact(&mut bytes) {
            Ok(()) => Ok(Some(raw_input_event(&bytes))),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ENODEV) => {
                self.active = false;
                tracing::warn!(
                    "level=warn event=evdev_device_removed path={}",
                    self.path.display()
                );
                Ok(None)
            }
            Err(error) => Err(evdev_error(
                ErrorPhase::Trigger,
                format!(
                    "cannot read evdev input device '{}': {error}",
                    self.path.display()
                ),
                Some(self.path.as_path()),
            )),
        }
    }

    fn set_grabbed(&mut self, grabbed: bool) -> Result<(), DiagnosableError> {
        if self.grabbed == grabbed {
            return Ok(());
        }
        let value: libc::c_int = if grabbed { 1 } else { 0 };
        // Safety: EVIOCGRAB only toggles exclusive delivery for this owned
        // input device descriptor. The pointer is valid for the duration of
        // the ioctl call.
        let result = unsafe { libc::ioctl(self.file.as_raw_fd(), eviocgrab(), value) };
        if result < 0 {
            return Err(evdev_error(
                ErrorPhase::Registration,
                format!(
                    "cannot {} evdev input device '{}': {}",
                    if grabbed { "grab" } else { "release" },
                    self.path.display(),
                    io::Error::last_os_error()
                ),
                Some(self.path.as_path()),
            ));
        }
        self.grabbed = grabbed;
        Ok(())
    }
}

#[cfg(target_pointer_width = "64")]
const INPUT_EVENT_SIZE: usize = 24;
#[cfg(target_pointer_width = "32")]
const INPUT_EVENT_SIZE: usize = 16;

#[cfg(test)]
fn decode_input_event(bytes: &[u8; INPUT_EVENT_SIZE]) -> Option<MotionInputEvent> {
    decode_raw_input_event(&raw_input_event(bytes))
}

fn raw_input_event(bytes: &[u8; INPUT_EVENT_SIZE]) -> RawInputEvent {
    let event_type = u16::from_ne_bytes([bytes[TIMEVAL_SIZE], bytes[TIMEVAL_SIZE + 1]]);
    let code = u16::from_ne_bytes([bytes[TIMEVAL_SIZE + 2], bytes[TIMEVAL_SIZE + 3]]);
    let value = i32::from_ne_bytes([
        bytes[TIMEVAL_SIZE + 4],
        bytes[TIMEVAL_SIZE + 5],
        bytes[TIMEVAL_SIZE + 6],
        bytes[TIMEVAL_SIZE + 7],
    ]);
    RawInputEvent {
        event_type,
        code,
        value,
    }
}

fn decode_raw_input_event(raw: &RawInputEvent) -> Option<MotionInputEvent> {
    let event_type = raw.event_type;
    let code = raw.code;
    let value = raw.value;
    if event_type == EV_REL && code == REL_WHEEL {
        return match value.cmp(&0) {
            std::cmp::Ordering::Greater => Some(MotionInputEvent::pressed(MotionToken::Wheel(
                WheelDirection::Up,
            ))),
            std::cmp::Ordering::Less => Some(MotionInputEvent::pressed(MotionToken::Wheel(
                WheelDirection::Down,
            ))),
            std::cmp::Ordering::Equal => None,
        };
    }
    if event_type != EV_KEY {
        return None;
    }
    let state = match value {
        0 => MotionInputState::Released,
        1 => MotionInputState::Pressed,
        2 => return None,
        _ => return None,
    };
    evdev_code_to_motion_token(code).map(|token| MotionInputEvent { token, state })
}

#[cfg(target_pointer_width = "64")]
const TIMEVAL_SIZE: usize = 16;
#[cfg(target_pointer_width = "32")]
const TIMEVAL_SIZE: usize = 8;

fn evdev_code_to_motion_token(code: u16) -> Option<MotionToken> {
    match code {
        KEY_F => Some(MotionToken::Key("f".to_string())),
        KEY_F1..=KEY_F10 => Some(MotionToken::Key(format!("F{}", code - KEY_F1 + 1))),
        KEY_F11..=KEY_F12 => Some(MotionToken::Key(format!("F{}", code - KEY_F11 + 11))),
        KEY_F13..=KEY_F24 => Some(MotionToken::Key(format!("F{}", code - KEY_F13 + 13))),
        BTN_LEFT => Some(MotionToken::MouseButton(MouseButton::Left)),
        BTN_RIGHT => Some(MotionToken::MouseButton(MouseButton::Right)),
        BTN_MIDDLE => Some(MotionToken::MouseButton(MouseButton::Middle)),
        _ => None,
    }
}

fn evdev_error(
    phase: ErrorPhase,
    message: impl Into<String>,
    source: Option<&Path>,
) -> DiagnosableError {
    let error = DiagnosableError::new(phase, message)
        .with_source(source.map_or_else(|| "evdev".to_string(), |path| path.display().to_string()))
        .with_remediation(
            "configure explicit evdev devices and grant this user read access to those device files",
        );
    error.with_capability(signal_auras_core::Capability::CompositePointerObservation)
}

fn udev_error(error: io::Error) -> DiagnosableError {
    evdev_error(
        ErrorPhase::Registration,
        format!("cannot initialize udev input monitor: {error}"),
        None,
    )
}

trait NonblockingFile {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
}

impl NonblockingFile for File {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let fd = self.as_raw_fd();
        // Safety: fcntl only reads/modifies flags for this owned file descriptor.
        let current = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if current < 0 {
            return Err(io::Error::last_os_error());
        }
        let next = if nonblocking {
            current | libc::O_NONBLOCK
        } else {
            current & !libc::O_NONBLOCK
        };
        // Safety: fcntl sets flags for this owned file descriptor.
        let result = unsafe { libc::fcntl(fd, libc::F_SETFL, next) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn eviocgrab() -> libc::c_ulong {
    ioctl_write_int(b'E', 0x90)
}

fn device_name(file: &File) -> Option<String> {
    let mut bytes = [0u8; 256];
    // Safety: EVIOCGNAME writes at most bytes.len() bytes into this valid
    // stack buffer for the owned input device descriptor.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            eviocgname(bytes.len()),
            bytes.as_mut_ptr(),
        )
    };
    if result < 0 {
        return None;
    }
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[..length]).into_owned())
}

fn device_is_pointer_capable(file: &File) -> bool {
    device_has_code(file, EV_REL, REL_X)
        || device_has_code(file, EV_REL, REL_Y)
        || device_has_code(file, EV_REL, REL_WHEEL)
        || device_has_code(file, EV_KEY, BTN_LEFT)
        || device_has_code(file, EV_KEY, BTN_RIGHT)
        || device_has_code(file, EV_KEY, BTN_MIDDLE)
}

fn device_has_code(file: &File, event_type: u16, code: u16) -> bool {
    let mut bytes = [0u8; 96];
    // Safety: EVIOCGBIT writes at most bytes.len() bytes into this valid
    // stack buffer for the owned input device descriptor.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            eviocgbit(event_type, bytes.len()),
            bytes.as_mut_ptr(),
        )
    };
    if result <= 0 {
        return false;
    }
    let index = usize::from(code / 8);
    let bit = (code % 8) as u8;
    bytes
        .get(index)
        .is_some_and(|byte| byte & (1u8 << bit) != 0)
}

fn eviocgname(length: usize) -> libc::c_ulong {
    ioctl_read(b'E', 0x06, length)
}

fn eviocgbit(event_type: u16, length: usize) -> libc::c_ulong {
    ioctl_read(
        b'E',
        0x20 + u8::try_from(event_type).unwrap_or(u8::MAX),
        length,
    )
}

fn ioctl_write_int(kind: u8, number: u8) -> libc::c_ulong {
    const IOC_WRITE: libc::c_ulong = 1;
    const IOC_NRBITS: libc::c_ulong = 8;
    const IOC_TYPEBITS: libc::c_ulong = 8;
    const IOC_SIZEBITS: libc::c_ulong = 14;
    const IOC_NRSHIFT: libc::c_ulong = 0;
    const IOC_TYPESHIFT: libc::c_ulong = IOC_NRSHIFT + IOC_NRBITS;
    const IOC_SIZESHIFT: libc::c_ulong = IOC_TYPESHIFT + IOC_TYPEBITS;
    const IOC_DIRSHIFT: libc::c_ulong = IOC_SIZESHIFT + IOC_SIZEBITS;
    (IOC_WRITE << IOC_DIRSHIFT)
        | ((kind as libc::c_ulong) << IOC_TYPESHIFT)
        | ((number as libc::c_ulong) << IOC_NRSHIFT)
        | ((std::mem::size_of::<libc::c_int>() as libc::c_ulong) << IOC_SIZESHIFT)
}

fn ioctl_read(kind: u8, number: u8, size: usize) -> libc::c_ulong {
    const IOC_READ: libc::c_ulong = 2;
    const IOC_NRBITS: libc::c_ulong = 8;
    const IOC_TYPEBITS: libc::c_ulong = 8;
    const IOC_SIZEBITS: libc::c_ulong = 14;
    const IOC_NRSHIFT: libc::c_ulong = 0;
    const IOC_TYPESHIFT: libc::c_ulong = IOC_NRSHIFT + IOC_NRBITS;
    const IOC_SIZESHIFT: libc::c_ulong = IOC_TYPESHIFT + IOC_TYPEBITS;
    const IOC_DIRSHIFT: libc::c_ulong = IOC_SIZESHIFT + IOC_SIZEBITS;
    (IOC_READ << IOC_DIRSHIFT)
        | ((kind as libc::c_ulong) << IOC_TYPESHIFT)
        | ((number as libc::c_ulong) << IOC_NRSHIFT)
        | ((size as libc::c_ulong) << IOC_SIZESHIFT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn decodes_keyboard_and_mouse_evdev_events() {
        assert_eq!(
            decode_input_event(&input_event_bytes(EV_KEY, KEY_F13, 1)).unwrap(),
            MotionInputEvent::pressed(MotionToken::Key("F13".to_string()))
        );
        assert_eq!(
            decode_input_event(&input_event_bytes(EV_KEY, BTN_LEFT, 0)).unwrap(),
            MotionInputEvent::released(MotionToken::MouseButton(MouseButton::Left))
        );
    }

    #[test]
    fn decodes_mouse_wheel_evdev_events() {
        assert_eq!(
            decode_input_event(&input_event_bytes(EV_REL, REL_WHEEL, 1)).unwrap(),
            MotionInputEvent::pressed(MotionToken::Wheel(WheelDirection::Up))
        );
        assert_eq!(
            decode_input_event(&input_event_bytes(EV_REL, REL_WHEEL, -1)).unwrap(),
            MotionInputEvent::pressed(MotionToken::Wheel(WheelDirection::Down))
        );
    }

    #[test]
    fn ignores_unmapped_or_non_key_events() {
        assert!(decode_input_event(&input_event_bytes(0x02, KEY_F, 1)).is_none());
        assert!(decode_input_event(&input_event_bytes(EV_KEY, 999, 1)).is_none());
    }

    #[test]
    fn ignores_evdev_key_auto_repeat_events() {
        assert!(decode_input_event(&input_event_bytes(EV_KEY, KEY_F13, 2)).is_none());
    }

    #[test]
    fn maps_function_key_range() {
        assert_eq!(
            evdev_code_to_motion_token(KEY_F1),
            Some(MotionToken::Key("F1".to_string()))
        );
        assert_eq!(
            evdev_code_to_motion_token(KEY_F24),
            Some(MotionToken::Key("F24".to_string()))
        );
    }

    #[test]
    fn all_device_discovery_uses_event_prefix_filter() {
        assert!(is_event_device_path(Path::new("/dev/input/event0")));
        assert!(is_event_device_path(Path::new("/dev/input/event12")));
        assert!(!is_event_device_path(Path::new("/dev/input/mouse0")));
    }

    #[test]
    fn wait_reads_ready_events_with_source_metadata() {
        let path = temp_event_device("ready");
        std::fs::write(&path, input_event_bytes(EV_KEY, KEY_F13, 1)).unwrap();
        let mut provider = EvdevObservationProvider::open(
            [path.clone()],
            InputProviderMode::Observe,
            Some(MotionToken::Key("F13".to_string())),
            false,
        )
        .unwrap();

        let event = provider
            .wait_next_observed_motion_event(Duration::from_millis(1))
            .unwrap()
            .unwrap();

        assert_eq!(event.source, path);
        assert_eq!(event.event, MotionInputEvent::pressed(MotionToken::Leader));
    }

    #[test]
    fn all_devices_rescan_adds_new_paths() {
        let first = temp_event_device("first");
        let second = temp_event_device("second");
        std::fs::write(&first, []).unwrap();
        std::fs::write(&second, []).unwrap();
        let mut provider =
            EvdevObservationProvider::open([first], InputProviderMode::Observe, None, true)
                .unwrap();

        assert_eq!(provider.device_count(), 1);
        provider.rescan_devices([second]);

        assert_eq!(provider.device_count(), 2);
        assert_eq!(provider.active_device_count(), 2);
    }

    #[test]
    fn all_devices_rescan_remembers_skipped_paths_until_they_open() {
        let first = temp_event_device("first");
        let denied = PathBuf::from("/tmp/signal-auras-missing-denied-device.event");
        let opened_later = temp_event_device("opened-later");
        std::fs::write(&first, []).unwrap();
        let mut provider =
            EvdevObservationProvider::open([first], InputProviderMode::Observe, None, true)
                .unwrap();

        provider.rescan_devices([denied.clone()]);
        provider.rescan_devices([denied.clone()]);

        assert_eq!(provider.skipped_paths.len(), 1);
        assert!(provider.skipped_paths.contains(&denied));

        std::fs::write(&opened_later, []).unwrap();
        provider.rescan_devices([opened_later.clone()]);

        assert!(!provider.skipped_paths.contains(&denied));
        assert_eq!(provider.device_count(), 2);
    }

    #[test]
    fn drains_large_queued_input_set_fairly_across_devices() {
        const DEVICE_COUNT: usize = 8;
        const EVENTS_PER_DEVICE: usize = 128;
        let paths = (0..DEVICE_COUNT)
            .map(|index| {
                let path = temp_event_device(&format!("stress-{index}"));
                let mut bytes = Vec::with_capacity(EVENTS_PER_DEVICE * INPUT_EVENT_SIZE);
                for event_index in 0..EVENTS_PER_DEVICE {
                    let code = if event_index % 2 == 0 {
                        KEY_F13
                    } else {
                        BTN_LEFT
                    };
                    bytes.extend_from_slice(&input_event_bytes(EV_KEY, code, 1));
                }
                std::fs::write(&path, bytes).unwrap();
                path
            })
            .collect::<Vec<_>>();
        let mut provider = EvdevObservationProvider::open(
            paths.clone(),
            InputProviderMode::Observe,
            Some(MotionToken::Key("F13".to_string())),
            false,
        )
        .unwrap();
        let start = Instant::now();
        let mut by_source = BTreeMap::new();

        for _ in 0..(DEVICE_COUNT * EVENTS_PER_DEVICE) {
            let event = provider.next_observed_motion_event().unwrap().unwrap();
            *by_source.entry(event.source).or_insert(0usize) += 1;
        }

        assert!(provider.next_observed_motion_event().unwrap().is_none());
        assert_eq!(by_source.len(), DEVICE_COUNT);
        assert!(by_source.values().all(|count| *count == EVENTS_PER_DEVICE));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "queued simulated input should drain quickly"
        );
    }

    fn input_event_bytes(event_type: u16, code: u16, value: i32) -> [u8; INPUT_EVENT_SIZE] {
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        bytes[TIMEVAL_SIZE..TIMEVAL_SIZE + 2].copy_from_slice(&event_type.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 2..TIMEVAL_SIZE + 4].copy_from_slice(&code.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 4..TIMEVAL_SIZE + 8].copy_from_slice(&value.to_ne_bytes());
        bytes
    }

    fn temp_event_device(label: &str) -> PathBuf {
        static NEXT_FILE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        let sequence = NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        path.push(format!(
            "signal-auras-evdev-{label}-{}-{sequence}.event",
            std::process::id()
        ));
        path
    }
}
