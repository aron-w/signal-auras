# Lua API Contract: v1 Hotkey Runner

## Script Shape

Lua scripts must return one table:

```lua
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
```

## Available Constructors

- `macro { ... }`: creates one ordered macro definition.
- `key "<key-name>"`: creates a key press action.
- `text "<string>"`: creates a text input action.
- `delay <milliseconds>`: creates a delay action.

## Scope Rules

- `scope.processes` may declare one or more process names.
- Missing `scope` causes terminal prompt selection.
- Script-declared global scope is not part of v1.
- Empty process lists are invalid.

## Sandbox Rules

The Lua environment must not expose ambient access to:

- filesystem
- network
- process APIs
- shell execution
- environment variables
- compositor APIs
- active process metadata
- global input capture
- synthesized input
- host application state

The only host-provided API for v1 is the constructor surface documented above.

## Validation Errors

The host rejects scripts before hotkey registration when:

- the script is invalid Lua
- the script returns a non-table value
- `hotkeys` is absent or empty
- a hotkey identifier is duplicated or unsupported
- a macro is empty
- a macro action is unsupported or malformed
- a process name is empty or malformed
- the script attempts to use unavailable ambient APIs

## Versioning

This contract is the v1 Lua script API. Breaking changes to accepted script shape, constructors, or behavior require migration notes and a major script API version change.
