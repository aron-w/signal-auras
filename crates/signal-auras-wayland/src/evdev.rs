use signal_auras_core::{
    DiagnosableError, ErrorPhase, InputProviderMode, MotionInputEvent, MotionInputState,
    MotionToken, MouseButton,
};
use std::{
    fs::{self, File},
    io::{self, Read},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

const EV_KEY: u16 = 0x01;
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
}

impl EvdevObservationProvider {
    pub fn open(
        devices: impl IntoIterator<Item = PathBuf>,
        mode: InputProviderMode,
        leader: Option<MotionToken>,
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
        let mut provider = Self {
            devices,
            leader,
            grabbed: false,
        };
        if mode == InputProviderMode::Grab {
            provider.grab_all()?;
        }
        Ok(provider)
    }

    pub fn next_motion_event(&mut self) -> Result<Option<MotionInputEvent>, DiagnosableError> {
        for device in &mut self.devices {
            if let Some(mut event) = device.next_motion_event()? {
                if self
                    .leader
                    .as_ref()
                    .is_some_and(|leader| event.token == *leader)
                {
                    event.token = MotionToken::Leader;
                }
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn active_device_count(&self) -> usize {
        self.devices.iter().filter(|device| device.active).count()
    }

    fn grab_all(&mut self) -> Result<(), DiagnosableError> {
        for device in &self.devices {
            device.set_grabbed(true)?;
        }
        self.grabbed = true;
        Ok(())
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
            for device in &self.devices {
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
        })
    }

    fn next_motion_event(&mut self) -> Result<Option<MotionInputEvent>, DiagnosableError> {
        if !self.active {
            return Ok(None);
        }
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        match self.file.read_exact(&mut bytes) {
            Ok(()) => Ok(decode_input_event(&bytes)),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ENODEV) => {
                self.active = false;
                eprintln!(
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

    fn set_grabbed(&self, grabbed: bool) -> Result<(), DiagnosableError> {
        let value: libc::c_int = if grabbed { 1 } else { 0 };
        // Safety: EVIOCGRAB only toggles exclusive delivery for this owned
        // input device descriptor. The pointer is valid for the duration of
        // the ioctl call.
        let result = unsafe { libc::ioctl(self.file.as_raw_fd(), eviocgrab(), &value) };
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
        Ok(())
    }
}

#[cfg(target_pointer_width = "64")]
const INPUT_EVENT_SIZE: usize = 24;
#[cfg(target_pointer_width = "32")]
const INPUT_EVENT_SIZE: usize = 16;

fn decode_input_event(bytes: &[u8; INPUT_EVENT_SIZE]) -> Option<MotionInputEvent> {
    let event_type = u16::from_ne_bytes([bytes[TIMEVAL_SIZE], bytes[TIMEVAL_SIZE + 1]]);
    if event_type != EV_KEY {
        return None;
    }
    let code = u16::from_ne_bytes([bytes[TIMEVAL_SIZE + 2], bytes[TIMEVAL_SIZE + 3]]);
    let value = i32::from_ne_bytes([
        bytes[TIMEVAL_SIZE + 4],
        bytes[TIMEVAL_SIZE + 5],
        bytes[TIMEVAL_SIZE + 6],
        bytes[TIMEVAL_SIZE + 7],
    ]);
    let state = match value {
        0 => MotionInputState::Released,
        1 | 2 => MotionInputState::Pressed,
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn ignores_unmapped_or_non_key_events() {
        assert!(decode_input_event(&input_event_bytes(0x02, KEY_F, 1)).is_none());
        assert!(decode_input_event(&input_event_bytes(EV_KEY, 999, 1)).is_none());
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

    fn input_event_bytes(event_type: u16, code: u16, value: i32) -> [u8; INPUT_EVENT_SIZE] {
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        bytes[TIMEVAL_SIZE..TIMEVAL_SIZE + 2].copy_from_slice(&event_type.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 2..TIMEVAL_SIZE + 4].copy_from_slice(&code.to_ne_bytes());
        bytes[TIMEVAL_SIZE + 4..TIMEVAL_SIZE + 8].copy_from_slice(&value.to_ne_bytes());
        bytes
    }
}
