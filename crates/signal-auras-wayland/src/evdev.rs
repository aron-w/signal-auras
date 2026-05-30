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

pub const SIGNAL_AURAS_UINPUT_DEVICE_NAME: &str = crate::uinput::UINPUT_DEVICE_NAME;

#[derive(Debug)]
pub struct EvdevObservationProvider {
    devices: Vec<EvdevDevice>,
    configured_paths: Vec<PathBuf>,
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
        let configured_paths = devices.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut selected_paths = Vec::new();
        let mut opened_devices = Vec::new();
        let mut skipped_paths = BTreeSet::new();
        for path in &configured_paths {
            if !seen.insert(path.clone()) {
                if all_devices {
                    skipped_paths.insert(path.clone());
                    tracing::warn!(
                        "level=warn event=evdev_device_skipped path={} reason=duplicate",
                        path.display()
                    );
                    continue;
                }
                return Err(evdev_error(
                    ErrorPhase::Registration,
                    format!("duplicate selected evdev input device '{}'", path.display()),
                    Some(path),
                ));
            }
            selected_paths.push(path.clone());
            match EvdevDevice::open(path.clone()) {
                Ok(device) => opened_devices.push(device),
                Err(error) if all_devices => {
                    skipped_paths.insert(path.clone());
                    tracing::warn!(
                        "level=warn event=evdev_device_skipped path={} error={}",
                        path.display(),
                        error
                    );
                }
                Err(error) => return Err(error),
            }
        }
        if opened_devices.is_empty() {
            return Err(evdev_error(
                ErrorPhase::Registration,
                if all_devices {
                    "no usable evdev input devices were found for devices = \"all\""
                } else {
                    "no usable evdev input devices were found in the selected device paths"
                },
                None,
            ));
        }
        let monitor_enabled = all_devices || !opened_devices.is_empty();
        let provider = Self {
            devices: opened_devices,
            configured_paths: if all_devices {
                Vec::new()
            } else {
                selected_paths
            },
            leader,
            grabbed: false,
            next_device_index: 0,
            all_devices,
            mode,
            last_rescan: Instant::now(),
            rescan_interval: Duration::from_secs(1),
            skipped_paths,
            udev_monitor: if monitor_enabled {
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
                    kernel_timestamp: raw.kernel_timestamp,
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
            self.rescan_devices_if_due()?;
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
            self.rescan_devices_if_due()?;
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

    pub fn rescan_devices_if_due(&mut self) -> Result<(), DiagnosableError> {
        if self.last_rescan.elapsed() < self.rescan_interval {
            return Ok(());
        }
        self.last_rescan = Instant::now();
        if self.all_devices {
            let devices = discover_event_devices()?;
            self.rescan_devices(devices);
        } else {
            self.rescan_configured_devices();
        }
        Ok(())
    }

    pub fn rescan_devices(&mut self, devices: impl IntoIterator<Item = PathBuf>) {
        if !self.all_devices {
            return;
        }
        self.rescan_known_devices(devices);
    }

    pub fn rescan_configured_devices(&mut self) {
        if self.all_devices {
            return;
        }
        self.rescan_known_devices(self.configured_paths.clone());
    }

    fn rescan_known_devices(&mut self, devices: impl IntoIterator<Item = PathBuf>) {
        let discovered = devices.into_iter().collect::<BTreeSet<_>>();
        for path in &discovered {
            self.add_or_reopen_device(path.clone());
        }
        let active_paths = self
            .devices
            .iter()
            .filter(|device| device.active)
            .map(|device| device.path.clone())
            .collect::<BTreeSet<_>>();
        self.skipped_paths
            .retain(|path| discovered.contains(path) && !active_paths.contains(path));
    }

    fn add_or_reopen_device(&mut self, path: PathBuf) {
        if self
            .devices
            .iter()
            .any(|device| device.path == path && device.active)
        {
            self.skipped_paths.remove(&path);
            return;
        }
        match EvdevDevice::open(path.clone()) {
            Ok(mut device) => {
                if self.grabbed && self.mode == InputProviderMode::Grab && device.pointer_capable {
                    if let Err(error) = device.set_grabbed(true) {
                        tracing::warn!(
                            "level=warn event=evdev_device_grab_failed path={} error={}",
                            path.display(),
                            error
                        );
                        return;
                    }
                }
                self.skipped_paths.remove(&path);
                if let Some(existing) = self
                    .devices
                    .iter_mut()
                    .find(|existing| existing.path == path && !existing.active)
                {
                    tracing::info!(
                        "level=info event=evdev_device_reopened path={}",
                        path.display()
                    );
                    *existing = device;
                } else {
                    tracing::info!(
                        "level=info event=evdev_device_added path={}",
                        path.display()
                    );
                    self.devices.push(device);
                }
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
            if self.all_devices {
                self.reconcile_discovered_devices(discover_event_devices()?);
            } else {
                self.rescan_configured_devices();
            }
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
    pub kernel_timestamp: KernelEventTimestamp,
    pub observed_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInputEvent {
    pub event_type: u16,
    pub code: u16,
    pub value: i32,
    pub kernel_timestamp: KernelEventTimestamp,
}

impl RawInputEvent {
    pub fn should_passthrough(&self) -> bool {
        matches!(self.event_type, EV_SYN | EV_KEY | EV_REL)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelEventTimestamp {
    Unavailable,
    Monotonic(Duration),
}

impl KernelEventTimestamp {
    pub fn monotonic(timestamp: Duration) -> Self {
        Self::Monotonic(timestamp)
    }

    pub fn event_age_at(self, now: Duration) -> Option<Duration> {
        match self {
            Self::Unavailable => None,
            Self::Monotonic(timestamp) => now.checked_sub(timestamp),
        }
    }

    pub fn event_age_now(self) -> Option<Duration> {
        self.event_age_at(monotonic_clock_time()?)
    }

    fn from_timeval(seconds: i64, microseconds: i64) -> Self {
        if seconds < 0
            || !(0..1_000_000).contains(&microseconds)
            || (seconds == 0 && microseconds == 0)
        {
            return Self::Unavailable;
        }
        Self::Monotonic(Duration::new(seconds as u64, (microseconds as u32) * 1_000))
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
        if let Err(error) = set_monotonic_event_clock(&file) {
            tracing::debug!(
                "level=debug event=evdev_timestamp_clock_default path={} error={error}",
                path.display()
            );
        }
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
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EACCES | libc::EPERM | libc::EIO)
                ) =>
            {
                self.active = false;
                tracing::warn!(
                    "level=warn event=evdev_device_inactive path={} error={}",
                    self.path.display(),
                    error
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
    let (seconds, microseconds) = raw_input_timeval(bytes);
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
        kernel_timestamp: KernelEventTimestamp::from_timeval(seconds, microseconds),
    }
}

#[cfg(target_pointer_width = "64")]
fn raw_input_timeval(bytes: &[u8; INPUT_EVENT_SIZE]) -> (i64, i64) {
    let seconds = i64::from_ne_bytes(bytes[0..8].try_into().unwrap());
    let microseconds = i64::from_ne_bytes(bytes[8..16].try_into().unwrap());
    (seconds, microseconds)
}

#[cfg(target_pointer_width = "32")]
fn raw_input_timeval(bytes: &[u8; INPUT_EVENT_SIZE]) -> (i64, i64) {
    let seconds = i32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as i64;
    let microseconds = i32::from_ne_bytes(bytes[4..8].try_into().unwrap()) as i64;
    (seconds, microseconds)
}

fn monotonic_clock_time() -> Option<Duration> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // Safety: clock_gettime writes to a valid timespec pointer and does not
    // retain it after returning.
    let result = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) };
    if result < 0 || time.tv_sec < 0 || time.tv_nsec < 0 || time.tv_nsec >= 1_000_000_000 {
        return None;
    }
    Some(Duration::new(time.tv_sec as u64, time.tv_nsec as u32))
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

pub fn is_signal_auras_virtual_device_name(name: &str) -> bool {
    name == SIGNAL_AURAS_UINPUT_DEVICE_NAME
}

pub fn evdev_device_name(path: &Path) -> Option<String> {
    File::open(path).ok().and_then(|file| device_name(&file))
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

fn eviocsclockid() -> libc::c_ulong {
    ioctl_write_int(b'E', 0xa0)
}

fn set_monotonic_event_clock(file: &File) -> io::Result<()> {
    let clock_id: libc::c_int = libc::CLOCK_MONOTONIC;
    // Safety: EVIOCSCLOCKID reads the clock id from a valid pointer for the
    // duration of this ioctl call and only affects this input descriptor.
    let result = unsafe { libc::ioctl(file.as_raw_fd(), eviocsclockid(), &clock_id) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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
    fn parses_keyboard_kernel_timestamps() {
        let raw = raw_input_event(&input_event_bytes_at(EV_KEY, KEY_F13, 1, 42, 125_000));

        assert_eq!(
            raw.kernel_timestamp,
            KernelEventTimestamp::monotonic(Duration::new(42, 125_000_000))
        );
        assert_eq!(
            decode_raw_input_event(&raw),
            Some(MotionInputEvent::pressed(MotionToken::Key(
                "F13".to_string()
            )))
        );
    }

    #[test]
    fn parses_pointer_button_and_wheel_kernel_timestamps() {
        let button = raw_input_event(&input_event_bytes_at(EV_KEY, BTN_LEFT, 1, 43, 250_000));
        let wheel = raw_input_event(&input_event_bytes_at(EV_REL, REL_WHEEL, -1, 44, 375_000));

        assert_eq!(
            button.kernel_timestamp,
            KernelEventTimestamp::monotonic(Duration::new(43, 250_000_000))
        );
        assert_eq!(
            wheel.kernel_timestamp,
            KernelEventTimestamp::monotonic(Duration::new(44, 375_000_000))
        );
        assert_eq!(
            decode_raw_input_event(&button),
            Some(MotionInputEvent::pressed(MotionToken::MouseButton(
                MouseButton::Left
            )))
        );
        assert_eq!(
            decode_raw_input_event(&wheel),
            Some(MotionInputEvent::pressed(MotionToken::Wheel(
                WheelDirection::Down
            )))
        );
    }

    #[test]
    fn invalid_or_zero_kernel_timestamps_are_unavailable() {
        let zero = raw_input_event(&input_event_bytes_at(EV_KEY, KEY_F13, 1, 0, 0));
        let invalid_microseconds =
            raw_input_event(&input_event_bytes_at(EV_KEY, KEY_F13, 1, 10, 1_000_000));
        let negative_seconds = raw_input_event(&input_event_bytes_at(EV_KEY, KEY_F13, 1, -1, 0));

        assert_eq!(zero.kernel_timestamp, KernelEventTimestamp::Unavailable);
        assert_eq!(
            invalid_microseconds.kernel_timestamp,
            KernelEventTimestamp::Unavailable
        );
        assert_eq!(
            negative_seconds.kernel_timestamp,
            KernelEventTimestamp::Unavailable
        );
    }

    #[test]
    fn observed_input_events_preserve_kernel_and_userspace_timestamps() {
        let path = temp_event_device("observed-input-timestamp");
        let timestamp = KernelEventTimestamp::monotonic(Duration::new(45, 500_000_000));
        std::fs::write(&path, input_event_bytes_at(EV_KEY, KEY_F13, 1, 45, 500_000)).unwrap();
        let mut provider =
            EvdevObservationProvider::open([path.clone()], InputProviderMode::Observe, None, false)
                .unwrap();

        let before_read = Instant::now();
        let observed = provider.next_observed_input_event().unwrap().unwrap();

        assert_eq!(observed.source, path);
        assert!(observed.observed_at >= before_read);
        assert_eq!(observed.raw.kernel_timestamp, timestamp);
        assert_eq!(
            observed.event,
            Some(MotionInputEvent::pressed(MotionToken::Key(
                "F13".to_string()
            )))
        );
    }

    #[test]
    fn observed_motion_events_preserve_kernel_timestamps() {
        let path = temp_event_device("observed-motion-timestamp");
        let timestamp = KernelEventTimestamp::monotonic(Duration::new(46, 625_000_000));
        std::fs::write(
            &path,
            input_event_bytes_at(EV_KEY, BTN_LEFT, 1, 46, 625_000),
        )
        .unwrap();
        let mut provider =
            EvdevObservationProvider::open([path], InputProviderMode::Observe, None, false)
                .unwrap();

        let observed = provider.next_observed_motion_event().unwrap().unwrap();

        assert_eq!(observed.kernel_timestamp, timestamp);
        assert_eq!(
            observed.event,
            MotionInputEvent::pressed(MotionToken::MouseButton(MouseButton::Left))
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
    fn selected_duplicate_paths_fail_closed() {
        let path = temp_event_device("duplicate-selected");
        std::fs::write(&path, []).unwrap();

        let error = EvdevObservationProvider::open(
            [path.clone(), path],
            InputProviderMode::Observe,
            None,
            false,
        )
        .unwrap_err();

        assert!(error
            .message
            .contains("duplicate selected evdev input device"));
    }

    #[test]
    fn all_devices_startup_skips_unusable_candidates() {
        let readable = temp_event_device("readable-all");
        let missing = temp_event_device("missing-all");
        std::fs::write(&readable, input_event_bytes(EV_KEY, KEY_F13, 1)).unwrap();

        let mut provider = EvdevObservationProvider::open(
            [missing.clone(), readable.clone()],
            InputProviderMode::Observe,
            None,
            true,
        )
        .unwrap();

        assert_eq!(provider.device_count(), 1);
        assert_eq!(provider.active_device_count(), 1);
        assert!(provider.skipped_paths.contains(&missing));
        assert_eq!(
            provider
                .next_observed_motion_event()
                .unwrap()
                .unwrap()
                .source,
            readable
        );
    }

    #[test]
    fn all_devices_startup_fails_when_no_candidate_is_usable() {
        let missing = temp_event_device("missing-only-all");

        let error =
            EvdevObservationProvider::open([missing], InputProviderMode::Observe, None, true)
                .unwrap_err();

        assert!(error.message.contains("no usable evdev input devices"));
    }

    #[test]
    fn noisy_unsupported_events_do_not_starve_supported_input() {
        let path = temp_event_device("noisy");
        let mut bytes = Vec::new();
        for _ in 0..128 {
            bytes.extend_from_slice(&input_event_bytes(EV_REL, REL_X, 1));
        }
        bytes.extend_from_slice(&input_event_bytes(EV_KEY, BTN_LEFT, 1));
        std::fs::write(&path, bytes).unwrap();
        let mut provider =
            EvdevObservationProvider::open([path], InputProviderMode::Observe, None, false)
                .unwrap();

        let mut observed = None;
        for _ in 0..129 {
            observed = provider.next_observed_motion_event().unwrap();
            if observed.is_some() {
                break;
            }
        }

        assert_eq!(
            observed.unwrap().event,
            MotionInputEvent::pressed(MotionToken::MouseButton(MouseButton::Left))
        );
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
    fn explicit_configured_path_rescan_reopens_inactive_device() {
        let path = temp_event_device("selected-reopen");
        std::fs::write(&path, []).unwrap();
        let mut provider =
            EvdevObservationProvider::open([path.clone()], InputProviderMode::Observe, None, false)
                .unwrap();
        provider.devices[0].active = false;
        std::fs::write(&path, input_event_bytes(EV_KEY, BTN_LEFT, 1)).unwrap();

        provider.rescan_configured_devices();
        let event = provider.next_observed_motion_event().unwrap().unwrap();

        assert_eq!(provider.device_count(), 1);
        assert_eq!(provider.active_device_count(), 1);
        assert_eq!(event.source, path);
        assert_eq!(
            event.event,
            MotionInputEvent::pressed(MotionToken::MouseButton(MouseButton::Left))
        );
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

    #[test]
    fn identifies_own_virtual_device_name() {
        assert!(is_signal_auras_virtual_device_name(
            crate::uinput::UINPUT_DEVICE_NAME
        ));
        assert!(!is_signal_auras_virtual_device_name("ordinary keyboard"));
    }

    fn input_event_bytes(event_type: u16, code: u16, value: i32) -> [u8; INPUT_EVENT_SIZE] {
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        bytes[TIMEVAL_SIZE..TIMEVAL_SIZE + 2].copy_from_slice(&event_type.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 2..TIMEVAL_SIZE + 4].copy_from_slice(&code.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 4..TIMEVAL_SIZE + 8].copy_from_slice(&value.to_ne_bytes());
        bytes
    }

    fn input_event_bytes_at(
        event_type: u16,
        code: u16,
        value: i32,
        seconds: i64,
        microseconds: i64,
    ) -> [u8; INPUT_EVENT_SIZE] {
        let mut bytes = input_event_bytes(event_type, code, value);
        write_input_timeval(&mut bytes, seconds, microseconds);
        bytes
    }

    #[cfg(target_pointer_width = "64")]
    fn write_input_timeval(bytes: &mut [u8; INPUT_EVENT_SIZE], seconds: i64, microseconds: i64) {
        bytes[0..8].copy_from_slice(&seconds.to_ne_bytes());
        bytes[8..16].copy_from_slice(&microseconds.to_ne_bytes());
    }

    #[cfg(target_pointer_width = "32")]
    fn write_input_timeval(bytes: &mut [u8; INPUT_EVENT_SIZE], seconds: i64, microseconds: i64) {
        bytes[0..4].copy_from_slice(&i32::try_from(seconds).unwrap().to_ne_bytes());
        bytes[4..8].copy_from_slice(&i32::try_from(microseconds).unwrap().to_ne_bytes());
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
