# Contract: Signal Auras Lua Editor API

This contract describes the editor-facing Lua DSL metadata for Signal Auras scripts. It is not a runtime implementation and does not grant script capabilities.

## Globals

### `macro(actions)`

Creates an ordered macro definition from an array-like table of macro actions.

```lua
["F5"] = macro {
  key "Enter",
  text "/hideout",
  delay(50),
  key "Enter",
}
```

### `key(name)`

Declares a key press action.

- `name`: string key identifier accepted by the runtime validator.
- Returns: editor placeholder representing a macro action.

### `text(value)`

Declares a text input action.

- `value`: string text payload.
- Returns: editor placeholder representing a macro action.

### `delay(ms)`

Declares a delay action.

- `ms`: integer delay in milliseconds.
- Returns: editor placeholder representing a macro action.

## Boundaries

- This contract is for LuaLS/Neovim diagnostics and completion metadata only.
- Runtime semantics remain implemented by the Rust-backed Signal Auras Lua sandbox.
- The metadata must not expose filesystem, network, compositor, process, input, or other ambient capabilities to scripts.
