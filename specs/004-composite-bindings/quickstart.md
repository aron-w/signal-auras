# Quickstart: Composite Input Bindings

Create a Lua script with structured bindings:

```lua
return {
  bindings = {
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { wheel = "up" },
      },
      macro = macro {
        key "Left",
      },
    },
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { wheel = "down" },
      },
      macro = macro {
        key "Right",
      },
    },
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { button = "left" },
      },
      macro = macro {
        key "Alt+Right",
        text "hello world",
        key "Enter",
      },
    },
  },
}
```

Run:

```bash
nix develop -c cargo run -p signal-auras-cli -- run ./examples/composite-bindings.lua
```

Expected current provider behavior: consumed composite pointer bindings fail closed with a diagnosable capability error until KDE pointer observation and consumption support is implemented.

Verification:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo test
nix develop -c cargo clippy --all-targets -- -D warnings
nix flake check
```
