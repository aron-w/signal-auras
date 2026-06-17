# Contract: Lua Interactive Input Provider

## Supported Shape

```lua
input_provider = {
  backend = "evdev",
  mode = "grab",
  output = "uinput",
  devices = "interactive",
}
```

Controller-style `aura.configure({ input_provider = ... })` uses the same
provider shape.

## Semantics

- `devices = "interactive"` means selected evdev paths are resolved during
  startup from the mandatory runtime cache or terminal prompt.
- The Lua program does not receive cache paths, device fingerprints, selected
  permissions, or permission-helper authority.
- `devices = "interactive"` is distinct from `devices = "all"` and never
  persists broad discovery.

## Failure Behavior

- Non-evdev interactive providers fail validation.
- Non-interactive startup fails closed when no valid runtime cache exists.
- A stale cache fails closed or prompts before live input observation starts.
