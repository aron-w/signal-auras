use crate::{DiagnosableError, ErrorPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCategory {
    Letter,
    Number,
    Punctuation,
    Modifier,
    Function,
    Navigation,
    Editing,
    Keypad,
    System,
    Media,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyToken {
    name: String,
    evdev_code: u16,
    category: KeyCategory,
}

impl KeyToken {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "key name cannot be empty",
            ));
        }
        parse_key_token(value).ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported key '{value}'"),
            )
        })
    }

    pub fn from_evdev_code(code: u16) -> Option<Self> {
        key_entries()
            .find(|entry| entry.code == code)
            .map(KeyEntry::token)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn evdev_code(&self) -> u16 {
        self.evdev_code
    }

    pub fn category(&self) -> KeyCategory {
        self.category
    }

    pub fn aliases(&self) -> Vec<&'static str> {
        let mut aliases = KEY_ALIASES
            .iter()
            .filter_map(|alias| (alias.code == self.evdev_code).then_some(alias.name))
            .filter(|alias| !same_lookup_key(alias, &self.name))
            .collect::<Vec<_>>();
        if let Some(entry) = key_entries().find(|entry| entry.code == self.evdev_code) {
            if !same_lookup_key(entry.linux_name, &self.name)
                && aliases
                    .iter()
                    .all(|alias| !same_lookup_key(alias, entry.linux_name))
            {
                aliases.push(entry.linux_name);
            }
        }
        aliases
    }
}

impl std::fmt::Display for KeyToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Copy)]
struct KeyEntry {
    code: u16,
    linux_name: &'static str,
}

impl KeyEntry {
    fn token(self) -> KeyToken {
        KeyToken {
            name: canonical_name(self),
            evdev_code: self.code,
            category: category_for(self.linux_name),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct KeyAlias {
    name: &'static str,
    code: u16,
}

const GENERATED_KEY_TABLE: &str = include_str!("key_table.txt");

const KEY_ALIASES: &[KeyAlias] = &[
    KeyAlias {
        name: "Esc",
        code: 1,
    },
    KeyAlias {
        name: "Escape",
        code: 1,
    },
    KeyAlias {
        name: "Enter",
        code: 28,
    },
    KeyAlias {
        name: "Return",
        code: 28,
    },
    KeyAlias {
        name: "Ctrl",
        code: 29,
    },
    KeyAlias {
        name: "Control",
        code: 29,
    },
    KeyAlias {
        name: "Shift",
        code: 42,
    },
    KeyAlias {
        name: "Alt",
        code: 56,
    },
    KeyAlias {
        name: "Super",
        code: 125,
    },
    KeyAlias {
        name: "Meta",
        code: 125,
    },
    KeyAlias {
        name: "Delete",
        code: 111,
    },
    KeyAlias {
        name: "Del",
        code: 111,
    },
    KeyAlias {
        name: "PageUp",
        code: 104,
    },
    KeyAlias {
        name: "PageDown",
        code: 109,
    },
    KeyAlias {
        name: "PgUp",
        code: 104,
    },
    KeyAlias {
        name: "PgDn",
        code: 109,
    },
    KeyAlias {
        name: "VolumeUp",
        code: 115,
    },
    KeyAlias {
        name: "VolumeDown",
        code: 114,
    },
    KeyAlias {
        name: "MicMute",
        code: 248,
    },
    KeyAlias {
        name: "PlayPause",
        code: 164,
    },
    KeyAlias {
        name: "NextSong",
        code: 163,
    },
    KeyAlias {
        name: "PreviousSong",
        code: 165,
    },
    KeyAlias {
        name: "-",
        code: 12,
    },
    KeyAlias {
        name: "=",
        code: 13,
    },
    KeyAlias {
        name: "[",
        code: 26,
    },
    KeyAlias {
        name: "]",
        code: 27,
    },
    KeyAlias {
        name: ";",
        code: 39,
    },
    KeyAlias {
        name: "'",
        code: 40,
    },
    KeyAlias {
        name: "`",
        code: 41,
    },
    KeyAlias {
        name: "\\",
        code: 43,
    },
    KeyAlias {
        name: ",",
        code: 51,
    },
    KeyAlias {
        name: ".",
        code: 52,
    },
    KeyAlias {
        name: "/",
        code: 53,
    },
    KeyAlias {
        name: "*",
        code: 55,
    },
    KeyAlias {
        name: "+",
        code: 78,
    },
];

fn parse_key_token(value: &str) -> Option<KeyToken> {
    let lookup = lookup_key(value);
    for alias in KEY_ALIASES {
        if lookup_key(alias.name) == lookup {
            return KeyToken::from_evdev_code(alias.code);
        }
    }
    for entry in key_entries() {
        let token = entry.token();
        if lookup_key(token.name()) == lookup
            || lookup_key(entry.linux_name) == lookup
            || lookup_key(entry.linux_name.trim_start_matches("KEY_")) == lookup
        {
            return Some(token);
        }
    }
    None
}

fn key_entries() -> impl Iterator<Item = KeyEntry> {
    GENERATED_KEY_TABLE.lines().filter_map(|line| {
        let mut parts = line.split_whitespace();
        let code = parts.next().and_then(parse_code)?;
        let linux_name = parts.next()?;
        Some(KeyEntry { code, linux_name })
    })
}

fn parse_code(value: &str) -> Option<u16> {
    value
        .strip_prefix("0x")
        .and_then(|hex| u16::from_str_radix(hex, 16).ok())
        .or_else(|| value.parse::<u16>().ok())
}

fn canonical_name(entry: KeyEntry) -> String {
    let raw = entry.linux_name.trim_start_matches("KEY_");
    if let Some(alias) = KEY_ALIASES.iter().find(|alias| alias.code == entry.code) {
        return alias.name.to_string();
    }
    if let Some(digit) = single_digit_name(raw) {
        return digit.to_string();
    }
    if let Some(letter) = single_letter_name(raw) {
        return letter.to_ascii_lowercase().to_string();
    }
    if let Some(function) = raw.strip_prefix('F').and_then(function_number) {
        return format!("F{function}");
    }
    if let Some(keypad) = raw.strip_prefix("KP") {
        return format!("KP{}", title_compound(keypad));
    }
    title_compound(raw)
}

fn category_for(raw: &str) -> KeyCategory {
    let raw = raw.trim_start_matches("KEY_");
    if single_letter_name(raw).is_some() {
        return KeyCategory::Letter;
    }
    if single_digit_name(raw).is_some() {
        return KeyCategory::Number;
    }
    if raw.strip_prefix('F').and_then(function_number).is_some() {
        return KeyCategory::Function;
    }
    if raw.starts_with("KP") {
        return KeyCategory::Keypad;
    }
    if matches!(
        raw,
        "LEFTCTRL"
            | "RIGHTCTRL"
            | "LEFTSHIFT"
            | "RIGHTSHIFT"
            | "LEFTALT"
            | "RIGHTALT"
            | "LEFTMETA"
            | "RIGHTMETA"
            | "CAPSLOCK"
            | "NUMLOCK"
            | "SCROLLLOCK"
            | "FN"
    ) {
        return KeyCategory::Modifier;
    }
    if matches!(
        raw,
        "LEFT" | "RIGHT" | "UP" | "DOWN" | "HOME" | "END" | "PAGEUP" | "PAGEDOWN"
    ) {
        return KeyCategory::Navigation;
    }
    if matches!(
        raw,
        "ENTER"
            | "ESC"
            | "TAB"
            | "BACKSPACE"
            | "DELETE"
            | "INSERT"
            | "UNDO"
            | "COPY"
            | "PASTE"
            | "CUT"
            | "FIND"
            | "REDO"
    ) {
        return KeyCategory::Editing;
    }
    if matches!(
        raw,
        "MINUS"
            | "EQUAL"
            | "LEFTBRACE"
            | "RIGHTBRACE"
            | "SEMICOLON"
            | "APOSTROPHE"
            | "GRAVE"
            | "BACKSLASH"
            | "COMMA"
            | "DOT"
            | "SLASH"
            | "102ND"
            | "RO"
            | "YEN"
    ) {
        return KeyCategory::Punctuation;
    }
    if raw.contains("VOLUME")
        || raw.contains("PLAY")
        || raw.contains("PAUSE")
        || raw.contains("SONG")
        || raw.contains("MEDIA")
        || raw.contains("BRIGHTNESS")
        || raw.contains("MUTE")
        || raw.contains("WLAN")
        || raw.contains("BLUETOOTH")
        || raw.contains("KBDILLUM")
        || raw.contains("MICMUTE")
    {
        return KeyCategory::Media;
    }
    if matches!(
        raw,
        "POWER" | "POWER2" | "SLEEP" | "WAKEUP" | "SYSRQ" | "PAUSE" | "MENU" | "COMPOSE"
    ) {
        return KeyCategory::System;
    }
    KeyCategory::Unknown
}

fn single_digit_name(raw: &str) -> Option<char> {
    (raw.len() == 1)
        .then(|| raw.chars().next().unwrap())
        .filter(|character| character.is_ascii_digit())
}

fn single_letter_name(raw: &str) -> Option<char> {
    (raw.len() == 1)
        .then(|| raw.chars().next().unwrap())
        .filter(|character| character.is_ascii_uppercase())
}

fn function_number(raw: &str) -> Option<u8> {
    raw.parse::<u8>()
        .ok()
        .filter(|number| (1..=24).contains(number))
}

fn title_compound(raw: &str) -> String {
    match raw {
        "ESC" => "Esc".to_string(),
        "LEFTCTRL" => "LeftCtrl".to_string(),
        "RIGHTCTRL" => "RightCtrl".to_string(),
        "LEFTSHIFT" => "LeftShift".to_string(),
        "RIGHTSHIFT" => "RightShift".to_string(),
        "LEFTALT" => "LeftAlt".to_string(),
        "RIGHTALT" => "RightAlt".to_string(),
        "LEFTMETA" => "LeftMeta".to_string(),
        "RIGHTMETA" => "RightMeta".to_string(),
        "LEFTBRACE" => "LeftBracket".to_string(),
        "RIGHTBRACE" => "RightBracket".to_string(),
        "PAGEUP" => "PageUp".to_string(),
        "PAGEDOWN" => "PageDown".to_string(),
        "VOLUMEUP" => "VolumeUp".to_string(),
        "VOLUMEDOWN" => "VolumeDown".to_string(),
        "PLAYPAUSE" => "PlayPause".to_string(),
        "NEXTSONG" => "NextSong".to_string(),
        "PREVIOUSSONG" => "PreviousSong".to_string(),
        "BRIGHTNESSUP" => "BrightnessUp".to_string(),
        "BRIGHTNESSDOWN" => "BrightnessDown".to_string(),
        "MICMUTE" => "MicMute".to_string(),
        "KBDILLUMUP" => "KbdIllumUp".to_string(),
        "KBDILLUMDOWN" => "KbdIllumDown".to_string(),
        "KBDILLUMTOGGLE" => "KbdIllumToggle".to_string(),
        "KPPLUS" => "Plus".to_string(),
        "KPMINUS" => "Minus".to_string(),
        "KPASTERISK" => "Asterisk".to_string(),
        "KPSLASH" => "Slash".to_string(),
        "KPDOT" => "Dot".to_string(),
        "KPENTER" => "Enter".to_string(),
        "KPEQUAL" => "Equal".to_string(),
        _ => raw
            .split('_')
            .filter(|part| !part.is_empty())
            .map(title_word)
            .collect::<String>(),
    }
}

fn title_word(raw: &str) -> String {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut word = first.to_ascii_uppercase().to_string();
    word.extend(chars.map(|character| character.to_ascii_lowercase()));
    word
}

fn lookup_key(value: &str) -> String {
    let value = value.trim();
    if value.chars().count() == 1
        && value
            .chars()
            .all(|character| !character.is_ascii_alphanumeric())
    {
        return format!("punct:{value}");
    }
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn same_lookup_key(left: &str, right: &str) -> bool {
    lookup_key(left) == lookup_key(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_representative_key_categories() {
        assert_eq!(
            KeyToken::parse("a").unwrap().category(),
            KeyCategory::Letter
        );
        assert_eq!(
            KeyToken::parse("7").unwrap().category(),
            KeyCategory::Number
        );
        assert_eq!(
            KeyToken::parse("F24").unwrap().category(),
            KeyCategory::Function
        );
        assert_eq!(
            KeyToken::parse("PageUp").unwrap().category(),
            KeyCategory::Navigation
        );
        assert_eq!(
            KeyToken::parse("Backspace").unwrap().category(),
            KeyCategory::Editing
        );
        assert_eq!(
            KeyToken::parse("KPEnter").unwrap().category(),
            KeyCategory::Keypad
        );
        assert_eq!(
            KeyToken::parse("VolumeUp").unwrap().category(),
            KeyCategory::Media
        );
    }

    #[test]
    fn preserves_legacy_aliases() {
        assert_eq!(
            KeyToken::parse("Esc").unwrap(),
            KeyToken::parse("Escape").unwrap()
        );
        assert_eq!(
            KeyToken::parse("Enter").unwrap(),
            KeyToken::parse("Return").unwrap()
        );
        assert_eq!(
            KeyToken::parse("Delete").unwrap(),
            KeyToken::parse("Del").unwrap()
        );
        assert_eq!(
            KeyToken::parse("/").unwrap(),
            KeyToken::parse("Slash").unwrap()
        );
    }

    #[test]
    fn looks_up_by_evdev_code() {
        assert_eq!(KeyToken::from_evdev_code(104).unwrap().name(), "PageUp");
        assert_eq!(KeyToken::from_evdev_code(96).unwrap().name(), "KPEnter");
        assert_eq!(KeyToken::from_evdev_code(115).unwrap().name(), "VolumeUp");
        assert!(KeyToken::from_evdev_code(0x2ff).is_none());
    }

    #[test]
    fn exposes_aliases_for_diagnostics() {
        let enter = KeyToken::parse("Enter").unwrap();

        assert!(enter.aliases().contains(&"Return"));
        assert!(!enter.aliases().contains(&"Enter"));
    }
}
