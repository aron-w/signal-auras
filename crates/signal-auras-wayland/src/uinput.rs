use signal_auras_core::{
    Capability, DiagnosableError, ErrorPhase, MacroAction, MouseButton, SynthesizedInputRequest,
};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

const UINPUT_PATH: &str = "/dev/uinput";
const UINPUT_MAX_NAME_SIZE: usize = 80;
const BUS_USB: u16 = 0x03;
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0;
const KEY_ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_0: u16 = 11;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_P: u16 = 25;
const KEY_ENTER: u16 = 28;
const KEY_A: u16 = 30;
const KEY_L: u16 = 38;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_Z: u16 = 44;
const KEY_M: u16 = 50;
const KEY_SLASH: u16 = 53;
const KEY_SPACE: u16 = 57;
const KEY_F1: u16 = 59;
const KEY_F10: u16 = 68;
const KEY_F11: u16 = 87;
const KEY_F12: u16 = 88;
const KEY_LEFT: u16 = 105;
const KEY_RIGHT: u16 = 106;
const KEY_DELETE: u16 = 111;
const KEY_F13: u16 = 183;
const KEY_F24: u16 = 194;
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
                for character in text.chars() {
                    self.emit_character(character)?;
                }
                Ok(())
            }
            MacroAction::KeyPress { key } => self.emit_named_key(key),
            MacroAction::MouseClick { button } => {
                self.emit_key(mouse_button_code(*button), 1)?;
                self.emit_key(mouse_button_code(*button), 0)
            }
            MacroAction::Delay { .. } => Ok(()),
        }
    }

    fn configure(&self) -> Result<(), DiagnosableError> {
        self.ioctl(ioctl_set_evbit(), EV_KEY as libc::c_int)?;
        for code in supported_key_codes() {
            self.ioctl(ioctl_set_keybit(), code as libc::c_int)?;
        }
        Ok(())
    }

    fn create(&mut self) -> Result<(), DiagnosableError> {
        let mut setup = UinputSetup::named("signal-auras-uinput");
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

    fn emit_character(&mut self, character: char) -> Result<(), DiagnosableError> {
        let key = character_to_key(character).ok_or_else(|| {
            uinput_error(format!(
                "character '{character}' is unsupported by the uinput output path"
            ))
        })?;
        if key.shift {
            self.emit_key(KEY_LEFTSHIFT, 1)?;
        }
        self.emit_key(key.code, 1)?;
        self.emit_key(key.code, 0)?;
        if key.shift {
            self.emit_key(KEY_LEFTSHIFT, 0)?;
        }
        Ok(())
    }

    fn emit_named_key(&mut self, key: &str) -> Result<(), DiagnosableError> {
        let code = named_key_code(key).ok_or_else(|| {
            uinput_error(format!(
                "key '{key}' is unsupported by the uinput output path"
            ))
        })?;
        self.emit_key(code, 1)?;
        self.emit_key(code, 0)
    }

    fn emit_key(&mut self, code: u16, value: i32) -> Result<(), DiagnosableError> {
        self.write_event(EV_KEY, code, value)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)
    }

    fn write_event(
        &mut self,
        event_type: u16,
        code: u16,
        value: i32,
    ) -> Result<(), DiagnosableError> {
        let event = input_event(event_type, code, value);
        let bytes = event_as_bytes(&event);
        self.file.write_all(bytes).map_err(|error| {
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
    match key.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(KEY_ENTER),
        "tab" => Some(KEY_TAB),
        "escape" | "esc" => Some(KEY_ESC),
        "backspace" => Some(KEY_BACKSPACE),
        "delete" | "del" => Some(KEY_DELETE),
        "left" => Some(KEY_LEFT),
        "right" => Some(KEY_RIGHT),
        "space" => Some(KEY_SPACE),
        key if key.len() == 1 => key
            .chars()
            .next()
            .and_then(character_to_key)
            .map(|key| key.code),
        key => function_key_code(key),
    }
}

fn function_key_code(key: &str) -> Option<u16> {
    let number = key.strip_prefix('f')?.parse::<u16>().ok()?;
    match number {
        1..=10 => Some(KEY_F1 + number - 1),
        11..=12 => Some(KEY_F11 + number - 11),
        13..=24 => Some(KEY_F13 + number - 13),
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
    [
        KEY_ESC,
        KEY_BACKSPACE,
        KEY_TAB,
        KEY_ENTER,
        KEY_LEFTSHIFT,
        KEY_SPACE,
        KEY_SLASH,
        KEY_LEFT,
        KEY_RIGHT,
        KEY_DELETE,
        BTN_LEFT,
        BTN_RIGHT,
        BTN_MIDDLE,
    ]
    .into_iter()
    .chain(KEY_1..=KEY_0)
    .chain(KEY_Q..=KEY_P)
    .chain(KEY_A..=KEY_L)
    .chain(KEY_Z..=KEY_M)
    .chain(KEY_F1..=KEY_F10)
    .chain(KEY_F11..=KEY_F12)
    .chain(KEY_F13..=KEY_F24)
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
        assert_eq!(named_key_code("nope"), None);
    }

    #[test]
    fn encodes_input_event_shape() {
        let event = input_event(EV_KEY, KEY_A, 1);
        assert_eq!(
            event_as_bytes(&event).len(),
            std::mem::size_of::<InputEvent>()
        );
    }
}
