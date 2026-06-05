use mlua::{Lua, Value};
use signal_auras_core::{DiagnosableError, ErrorPhase};

const DENIED_GLOBALS: &[&str] = &[
    "io", "os", "package", "require", "debug", "dofile", "loadfile", "load", "socket", "luaposix",
];

const DECLARATIVE_DENIED_TOKENS: &[&str] = &[
    "io.", "os.", "require", "package", "debug", "dofile", "loadfile", "load(", "socket",
    "luaposix",
];

pub(crate) fn install_denied_globals(lua: &Lua) -> Result<(), DiagnosableError> {
    let globals = lua.globals();
    for denied in DENIED_GLOBALS {
        globals.set(*denied, Value::Nil).map_err(lua_error)?;
    }
    Ok(())
}

pub(crate) fn declarative_denied_tokens() -> &'static [&'static str] {
    DECLARATIVE_DENIED_TOKENS
}

pub(crate) fn denied_global_tokens() -> &'static [&'static str] {
    DENIED_GLOBALS
}

pub(crate) fn first_denied_token<'a>(source: &str, tokens: &'a [&'static str]) -> Option<&'a str> {
    tokens
        .iter()
        .copied()
        .find(|token| contains_denied_token(source, token))
}

pub(crate) fn contains_denied_token(source: &str, token: &str) -> bool {
    let bytes = source.as_bytes();
    let token_bytes = token.as_bytes();
    let mut index = 0usize;
    while index + token_bytes.len() <= bytes.len() {
        if bytes[index..].starts_with(b"--") {
            index = skip_line_comment(bytes, index + 2);
            continue;
        }
        if bytes[index..].starts_with(b"[[") {
            index = skip_long_string(bytes, index + 2);
            continue;
        }
        if matches!(bytes[index], b'"' | b'\'') {
            index = skip_quoted_string(bytes, index);
            continue;
        }
        if bytes[index..].starts_with(token_bytes)
            && (!is_identifier_token(token)
                || is_identifier_boundary(bytes, index, token_bytes.len()))
        {
            return true;
        }
        index += 1;
    }
    false
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_long_string(bytes: &[u8], mut index: usize) -> usize {
    while index + 1 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b']' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn skip_quoted_string(bytes: &[u8], mut index: usize) -> usize {
    let quote = bytes[index];
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn is_identifier_token(token: &str) -> bool {
    token
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn is_identifier_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(start + len);
    !before.is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && !after.is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn lua_error(error: mlua::Error) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::ScriptValidation, error.to_string())
}
