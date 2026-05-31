use crate::evdev::RawInputEvent;
use signal_auras_core::{
    Capability, DiagnosableError, ErrorPhase, KeyToken, MacroAction, MouseButton,
    SynthesizedInputRequest,
};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

pub(crate) const UINPUT_DEVICE_NAME: &str = "signal-auras-uinput";
const UINPUT_PATH: &str = "/dev/uinput";
const UINPUT_MAX_NAME_SIZE: usize = 80;
const BUS_USB: u16 = 0x03;
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const SYN_REPORT: u16 = 0;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const KEY_1: u16 = 2;
const KEY_0: u16 = 11;
const KEY_Q: u16 = 16;
#[cfg(test)]
const KEY_ENTER: u16 = 28;
const KEY_LEFTCTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_Z: u16 = 44;
const KEY_LEFTALT: u16 = 56;
const KEY_SLASH: u16 = 53;
const KEY_SPACE: u16 = 57;
const KEY_LEFTMETA: u16 = 125;
#[cfg(test)]
const KEY_LEFT: u16 = 105;
#[cfg(test)]
const KEY_RIGHT: u16 = 106;
#[cfg(test)]
const KEY_F13: u16 = 183;
const KEY_MAX: u16 = 0x2ff;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

#[derive(Debug)]
pub struct UinputOutputSession {
    file: File,
    path: PathBuf,
    active: bool,
}

impl UinputOutputSession {
    pub fn open() -> Result<Self, DiagnosableError> {
        Self::open_path(UINPUT_PATH)
    }

    fn open_path(path: impl AsRef<Path>) -> Result<Self, DiagnosableError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| uinput_error(format!("cannot open '{}': {error}", path.display())))?;
        let mut session = Self {
            file,
            path,
            active: false,
        };
        session.configure()?;
        session.create()?;
        Ok(session)
    }

    pub fn synthesize(
        &mut self,
        request: &SynthesizedInputRequest,
    ) -> Result<(), DiagnosableError> {
        match &request.action {
            MacroAction::TextInput { text } => {
                let mut events = Vec::new();
                for character in text.chars() {
                    self.push_character_events(character, &mut events)?;
                }
                self.write_events(&events)?;
                Ok(())
            }
            MacroAction::KeyPress { key } => self.emit_named_key(key),
            MacroAction::KeyDown { key } => self.emit_named_key_state(key, 1),
            MacroAction::KeyUp { key } => self.emit_named_key_state(key, 0),
            MacroAction::MouseClick { button } => {
                let mut events = Vec::new();
                events.extend(key_events(mouse_button_code(*button), 1));
                events.extend(key_events(mouse_button_code(*button), 0));
                self.write_events(&events)
            }
            MacroAction::Delay { .. } => Ok(()),
        }
    }

    pub fn passthrough_raw(&mut self, raw: &RawInputEvent) -> Result<(), DiagnosableError> {
        if !raw.should_passthrough() {
            return Ok(());
        }
        self.write_events(&[input_event(raw.event_type, raw.code, raw.value)])
    }

    fn configure(&self) -> Result<(), DiagnosableError> {
        self.ioctl(ioctl_set_evbit(), EV_KEY as libc::c_int)?;
        self.ioctl(ioctl_set_evbit(), EV_REL as libc::c_int)?;
        for code in supported_key_codes() {
            self.ioctl(ioctl_set_keybit(), code as libc::c_int)?;
        }
        for code in supported_relative_codes() {
            self.ioctl(ioctl_set_relbit(), code as libc::c_int)?;
        }
        Ok(())
    }

    fn create(&mut self) -> Result<(), DiagnosableError> {
        let mut setup = UinputSetup::named(UINPUT_DEVICE_NAME);
        setup.id = InputId {
            bustype: BUS_USB,
            vendor: 0x1209,
            product: 0x0001,
            version: 1,
        };
        self.ioctl_ptr(ioctl_dev_setup(), &setup)?;
        self.ioctl(ioctl_dev_create(), 0)?;
        self.active = true;
        Ok(())
    }

    fn push_character_events(
        &self,
        character: char,
        events: &mut Vec<InputEvent>,
    ) -> Result<(), DiagnosableError> {
        let key = character_to_key(character).ok_or_else(|| {
            uinput_error(format!(
                "character '{character}' is unsupported by the uinput output path"
            ))
        })?;
        if key.shift {
            events.extend(key_events(KEY_LEFTSHIFT, 1));
        }
        events.extend(key_events(key.code, 1));
        events.extend(key_events(key.code, 0));
        if key.shift {
            events.extend(key_events(KEY_LEFTSHIFT, 0));
        }
        Ok(())
    }

    fn emit_named_key(&mut self, key: &str) -> Result<(), DiagnosableError> {
        if key.contains('+') {
            let events = named_key_chord_events(key).ok_or_else(|| {
                uinput_error(format!(
                    "key '{key}' is unsupported by the uinput output path"
                ))
            })?;
            return self.write_events(&events);
        }
        let code = named_key_code(key).ok_or_else(|| {
            uinput_error(format!(
                "key '{key}' is unsupported by the uinput output path"
            ))
        })?;
        self.emit_key(code, 1)?;
        self.emit_key(code, 0)
    }

    fn emit_named_key_state(&mut self, key: &str, value: i32) -> Result<(), DiagnosableError> {
        if key.contains('+') {
            return Err(uinput_error(format!(
                "key chord '{key}' is unsupported for key_down/key_up by the uinput output path"
            )));
        }
        let code = named_key_code(key).ok_or_else(|| {
            uinput_error(format!(
                "key '{key}' is unsupported by the uinput output path"
            ))
        })?;
        self.emit_key(code, value)
    }

    fn emit_key(&mut self, code: u16, value: i32) -> Result<(), DiagnosableError> {
        self.write_events(&key_events(code, value))
    }

    fn write_events(&mut self, events: &[InputEvent]) -> Result<(), DiagnosableError> {
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(events));
        for event in events {
            bytes.extend_from_slice(event_as_bytes(event));
        }
        self.file.write_all(&bytes).map_err(|error| {
            uinput_error(format!(
                "cannot write uinput event to '{}': {error}",
                self.path.display()
            ))
        })
    }

    fn ioctl(&self, request: libc::c_ulong, value: libc::c_int) -> Result<(), DiagnosableError> {
        // Safety: ioctl is called for this owned uinput file descriptor with
        // integer request arguments defined by the uinput API.
        let result = unsafe { libc::ioctl(self.file.as_raw_fd(), request, value) };
        if result < 0 {
            return Err(uinput_error(format!(
                "uinput ioctl failed for '{}': {}",
                self.path.display(),
                io::Error::last_os_error()
            )));
        }
        Ok(())
    }

    fn ioctl_ptr<T>(&self, request: libc::c_ulong, value: &T) -> Result<(), DiagnosableError> {
        // Safety: ioctl reads the pointed setup structure during this call.
        let result = unsafe { libc::ioctl(self.file.as_raw_fd(), request, value) };
        if result < 0 {
            return Err(uinput_error(format!(
                "uinput setup ioctl failed for '{}': {}",
                self.path.display(),
                io::Error::last_os_error()
            )));
        }
        Ok(())
    }
}

fn key_events(code: u16, value: i32) -> [InputEvent; 2] {
    [
        input_event(EV_KEY, code, value),
        input_event(EV_SYN, SYN_REPORT, 0),
    ]
}

impl Drop for UinputOutputSession {
    fn drop(&mut self) {
        if self.active {
            // Safety: destroys the virtual device associated with this owned
            // uinput descriptor.
            let _ = unsafe { libc::ioctl(self.file.as_raw_fd(), ioctl_dev_destroy()) };
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyStroke {
    code: u16,
    shift: bool,
}

fn character_to_key(character: char) -> Option<KeyStroke> {
    let lower = character.to_ascii_lowercase();
    let shift = character.is_ascii_uppercase();
    let code = match lower {
        'a'..='z' => letter_key_code(lower)?,
        '0' => KEY_0,
        '1'..='9' => KEY_1 + (lower as u16 - '1' as u16),
        ' ' => KEY_SPACE,
        '/' => KEY_SLASH,
        _ => return None,
    };
    Some(KeyStroke { code, shift })
}

fn named_key_code(key: &str) -> Option<u16> {
    KeyToken::parse(key).ok().map(|key| key.evdev_code())
}

fn named_key_chord_events(key: &str) -> Option<Vec<InputEvent>> {
    let parts = key.split('+').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let (key, modifiers) = parts.split_last()?;
    let key_code = named_key_code(key)?;
    let modifier_codes = modifiers
        .iter()
        .map(|modifier| modifier_key_code(modifier))
        .collect::<Option<Vec<_>>>()?;
    let mut events = Vec::with_capacity((modifier_codes.len() * 2 + 1) * 2);
    for code in &modifier_codes {
        events.extend(key_events(*code, 1));
    }
    events.extend(key_events(key_code, 1));
    events.extend(key_events(key_code, 0));
    for code in modifier_codes.iter().rev() {
        events.extend(key_events(*code, 0));
    }
    Some(events)
}

fn modifier_key_code(modifier: &str) -> Option<u16> {
    match modifier {
        "Ctrl" => Some(KEY_LEFTCTRL),
        "Alt" => Some(KEY_LEFTALT),
        "Shift" => Some(KEY_LEFTSHIFT),
        "Super" => Some(KEY_LEFTMETA),
        _ => None,
    }
}

fn letter_key_code(character: char) -> Option<u16> {
    "qwertyuiop"
        .chars()
        .position(|candidate| candidate == character)
        .map(|index| KEY_Q + index as u16)
        .or_else(|| {
            "asdfghjkl"
                .chars()
                .position(|candidate| candidate == character)
                .map(|index| KEY_A + index as u16)
        })
        .or_else(|| {
            "zxcvbnm"
                .chars()
                .position(|candidate| candidate == character)
                .map(|index| KEY_Z + index as u16)
        })
}

fn mouse_button_code(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
    }
}

fn supported_key_codes() -> impl Iterator<Item = u16> {
    1..=KEY_MAX
}

fn supported_relative_codes() -> impl Iterator<Item = u16> {
    [REL_X, REL_Y, REL_HWHEEL, REL_WHEEL].into_iter()
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct UinputSetup {
    id: InputId,
    name: [u8; UINPUT_MAX_NAME_SIZE],
    ff_effects_max: u32,
}

impl UinputSetup {
    fn named(name: &str) -> Self {
        let mut setup = Self {
            id: InputId {
                bustype: 0,
                vendor: 0,
                product: 0,
                version: 0,
            },
            name: [0; UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };
        for (target, byte) in setup.name.iter_mut().zip(name.bytes()) {
            *target = byte;
        }
        setup
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

fn input_event(event_type: u16, code: u16, value: i32) -> InputEvent {
    InputEvent {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        type_: event_type,
        code,
        value,
    }
}

fn event_as_bytes(event: &InputEvent) -> &[u8] {
    let len = std::mem::size_of::<InputEvent>();
    // Safety: `event` is a plain repr(C) input_event value and the returned
    // slice is bounded to its exact in-memory size for immediate write().
    unsafe { std::slice::from_raw_parts(event as *const InputEvent as *const u8, len) }
}

fn ioctl_dev_create() -> libc::c_ulong {
    ioctl_none(b'U', 1)
}

fn ioctl_dev_destroy() -> libc::c_ulong {
    ioctl_none(b'U', 2)
}

fn ioctl_dev_setup() -> libc::c_ulong {
    ioctl_write::<UinputSetup>(b'U', 3)
}

fn ioctl_set_evbit() -> libc::c_ulong {
    ioctl_write_int(b'U', 100)
}

fn ioctl_set_keybit() -> libc::c_ulong {
    ioctl_write_int(b'U', 101)
}

fn ioctl_set_relbit() -> libc::c_ulong {
    ioctl_write_int(b'U', 102)
}

fn ioctl_none(kind: u8, number: u8) -> libc::c_ulong {
    ioctl(0, kind, number, 0)
}

fn ioctl_write<T>(kind: u8, number: u8) -> libc::c_ulong {
    ioctl(1, kind, number, std::mem::size_of::<T>() as libc::c_ulong)
}

fn ioctl_write_int(kind: u8, number: u8) -> libc::c_ulong {
    ioctl_write::<libc::c_int>(kind, number)
}

fn ioctl(dir: libc::c_ulong, kind: u8, number: u8, size: libc::c_ulong) -> libc::c_ulong {
    const IOC_NRBITS: libc::c_ulong = 8;
    const IOC_TYPEBITS: libc::c_ulong = 8;
    const IOC_SIZEBITS: libc::c_ulong = 14;
    const IOC_NRSHIFT: libc::c_ulong = 0;
    const IOC_TYPESHIFT: libc::c_ulong = IOC_NRSHIFT + IOC_NRBITS;
    const IOC_SIZESHIFT: libc::c_ulong = IOC_TYPESHIFT + IOC_TYPEBITS;
    const IOC_DIRSHIFT: libc::c_ulong = IOC_SIZESHIFT + IOC_SIZEBITS;
    (dir << IOC_DIRSHIFT)
        | ((kind as libc::c_ulong) << IOC_TYPESHIFT)
        | ((number as libc::c_ulong) << IOC_NRSHIFT)
        | (size << IOC_SIZESHIFT)
}

fn uinput_error(message: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::MacroExecution, message)
        .with_capability(Capability::SynthesizedInput)
        .with_source("uinput")
        .with_remediation("grant this user write access to /dev/uinput or use output = \"portal\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_text_needed_by_motion_examples() {
        assert_eq!(
            character_to_key('/'),
            Some(KeyStroke {
                code: KEY_SLASH,
                shift: false
            })
        );
        assert_eq!(
            character_to_key('A'),
            Some(KeyStroke {
                code: KEY_A,
                shift: true
            })
        );
    }

    #[test]
    fn maps_named_and_function_keys() {
        assert_eq!(named_key_code("Enter"), Some(KEY_ENTER));
        assert_eq!(named_key_code("Left"), Some(KEY_LEFT));
        assert_eq!(named_key_code("Right"), Some(KEY_RIGHT));
        assert_eq!(named_key_code("F13"), Some(KEY_F13));
        assert_eq!(named_key_code("PageUp"), Some(104));
        assert_eq!(named_key_code("KPEnter"), Some(96));
        assert_eq!(named_key_code("VolumeUp"), Some(115));
        assert_eq!(named_key_code("nope"), None);
    }

    #[test]
    fn maps_named_key_chords_without_substitution() {
        let events = named_key_chord_events("Alt+Right").unwrap();

        assert_eq!(events[0].code, KEY_LEFTALT);
        assert_eq!(events[0].value, 1);
        assert_eq!(events[2].code, KEY_RIGHT);
        assert_eq!(events[2].value, 1);
        assert_eq!(events[4].code, KEY_RIGHT);
        assert_eq!(events[4].value, 0);
        assert_eq!(events[6].code, KEY_LEFTALT);
        assert_eq!(events[6].value, 0);
        assert!(named_key_chord_events("Alt+Nope").is_none());
    }

    #[test]
    fn encodes_input_event_shape() {
        let event = input_event(EV_KEY, KEY_A, 1);
        assert_eq!(
            event_as_bytes(&event).len(),
            std::mem::size_of::<InputEvent>()
        );
    }

    #[test]
    fn advertises_pointer_axes_for_mouse_button_output() {
        let relative_codes = supported_relative_codes().collect::<Vec<_>>();

        assert_eq!(relative_codes, vec![REL_X, REL_Y, REL_HWHEEL, REL_WHEEL]);
        assert_eq!(ioctl_set_relbit(), ioctl_write_int(b'U', 102));
    }
}
