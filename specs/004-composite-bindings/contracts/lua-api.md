# Lua API Contract: Composite Input Bindings

Legacy hotkeys remain supported:

```lua
return {
  hotkeys = {
    ["F5"] = macro {
      key "Enter",
    },
  },
}
```

Structured bindings use a unified `bindings` list:

```lua
return {
  bindings = {
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { button = "left" },
      },
      mode = "consume",
      macro = macro {
        key "Alt+Right",
        text "hello world",
        key "Enter",
      },
    },
  },
}
```

## Trigger Fields

- `modifiers`: optional list of `Ctrl`, `Alt`, `Shift`, and `Super`
- `mouse.button`: `left`, `right`, or `middle`
- `mouse.wheel`: `up` or `down`
- `key`: keyboard trigger field for future-compatible structured bindings

Exactly one primary trigger is allowed: `mouse.button`, `mouse.wheel`, or `key`.

## Mode

- Missing `mode` defaults to `consume`.
- `mode = "passthrough"` allows the original input event to pass through.
- Consumed pointer bindings fail before activation when the provider cannot suppress the original pointer event.
