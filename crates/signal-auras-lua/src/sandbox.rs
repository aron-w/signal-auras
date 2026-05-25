use signal_auras_core::{
    DiagnosableError, ErrorPhase, HotkeyId, LuaAutomationConfiguration, MacroAction,
    MacroDefinition, ProcessName, ScriptScope,
};
use std::fs;
use std::path::Path;

const DENIED_TOKENS: &[&str] = &[
    "io.", "os.", "require", "package", "debug", "dofile", "loadfile", "load(", "socket",
    "luaposix",
];

pub fn load_lua_file(path: &Path) -> Result<LuaAutomationConfiguration, DiagnosableError> {
    let source = fs::read_to_string(path).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!("cannot read Lua file '{}': {error}", path.display()),
        )
    })?;
    load_lua_source(&source)
}

pub fn load_lua_source(source: &str) -> Result<LuaAutomationConfiguration, DiagnosableError> {
    for token in DENIED_TOKENS {
        if source.contains(token) {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("Lua sandbox denies ambient API token '{token}'"),
            ));
        }
    }
    if !source.contains("return") || !source.contains("hotkeys") {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "Lua script must return a table with hotkeys",
        ));
    }

    let scope = parse_scope(source)?;
    let hotkeys = parse_hotkeys(source)?;
    LuaAutomationConfiguration::new(scope, hotkeys)
}

fn parse_scope(source: &str) -> Result<Option<ScriptScope>, DiagnosableError> {
    let Some(scope_index) = source.find("scope") else {
        return Ok(None);
    };
    let scope_source = &source[scope_index..source.find("hotkeys").unwrap_or(source.len())];
    if scope_source.contains("global") {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "script-declared global scope is not accepted in v1",
        ));
    }
    let mut processes = Vec::new();
    for value in quoted_strings(scope_source) {
        processes.push(ProcessName::parse(value)?);
    }
    Ok(Some(ScriptScope::processes(processes)?))
}

fn parse_hotkeys(source: &str) -> Result<Vec<(HotkeyId, MacroDefinition)>, DiagnosableError> {
    let mut result = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("[\"") {
        cursor = &cursor[start + 2..];
        let Some(end) = cursor.find("\"]") else {
            break;
        };
        let hotkey = HotkeyId::parse(&cursor[..end])?;
        cursor = &cursor[end + 2..];
        let Some(macro_start) = cursor.find("macro") else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "hotkey must map to a macro",
            ));
        };
        cursor = &cursor[macro_start..];
        let Some(block_start) = cursor.find('{') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "macro must use table constructor",
            ));
        };
        let macro_body = &cursor[block_start + 1
            ..cursor[block_start + 1..]
                .find('}')
                .map(|end| block_start + 1 + end)
                .unwrap_or(cursor.len())];
        let actions = parse_actions(macro_body)?;
        result.push((hotkey, MacroDefinition::new(actions)?));
        cursor = &cursor[block_start + 1..];
    }
    Ok(result)
}

fn parse_actions(source: &str) -> Result<Vec<MacroAction>, DiagnosableError> {
    let mut actions = Vec::new();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(rest) = line.strip_prefix("key ") {
            actions.push(MacroAction::key(first_quoted(rest).ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "key action needs a string")
            })?)?);
        } else if let Some(rest) = line.strip_prefix("text ") {
            actions.push(MacroAction::text(first_quoted(rest).ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "text action needs a string")
            })?)?);
        } else if let Some(rest) = line.strip_prefix("delay ") {
            let number = rest
                .trim_matches(|c: char| c == ',' || c.is_whitespace())
                .parse::<u64>()
                .map_err(|_| {
                    DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "delay action needs milliseconds",
                    )
                })?;
            actions.push(MacroAction::delay(number)?);
        } else if line.starts_with('}') || line.starts_with('{') {
        } else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported macro action '{line}'"),
            ));
        }
    }
    Ok(actions)
}

fn quoted_strings(source: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut cursor = source;
    while let Some(value) = first_quoted(cursor) {
        values.push(value);
        let offset = cursor.find(value).unwrap_or(0) + value.len() + 1;
        cursor = &cursor[offset.min(cursor.len())..];
    }
    values
}

fn first_quoted(source: &str) -> Option<&str> {
    let start = source.find('"')? + 1;
    let end = source[start..].find('"')? + start;
    Some(&source[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_sample() {
        let source = r#"
            return {
              scope = { processes = { "poe2.exe" } },
              hotkeys = {
                ["F5"] = macro {
                  key "Enter",
                  text "/hideout",
                  delay 50,
                  key "Enter",
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        assert_eq!(config.hotkeys().len(), 1);
        assert!(config.scope.is_some());
    }

    #[test]
    fn denies_ambient_api() {
        assert!(load_lua_source("return { hotkeys = {}, x = os.getenv(\"HOME\") }").is_err());
    }
}
