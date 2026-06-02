# Quickstart: Lua Controller Runtime

## Example Controller

`main.lua`:

```lua
sa.import("motions")

sa.hotkey({
  trigger = "F5",
  scope = { processes = { "poe2.exe" } },
  callback = "hideout",
})
```

`motions.lua`:

```lua
sa.motion({
  trigger = "<Leader> x",
  mode = "passthrough",
  callback = "motion",
})
```

Loading `main.lua` collects both registrations and validates capabilities before any runtime activation.

## Imperative Callback Example

```lua
sa.press({
  requires_held = { "Ctrl" },
  trigger = "S",
  mode = "passthrough",
  capabilities = {
    "active_window_metadata",
    "window_activation",
    "synthesized_input",
    "timer",
  },
  callback = "reload_filterblade",
})

sa.callback("reload_filterblade", function()
  sa.sleep(100)
  local active = sa.window.active({ title = true })
  sa.log("active_title=" .. tostring(active.title))

  local filter = active.title and active.title:match("^FilterBlade%s+%-%s+(.-)%s+%-%s+FilterBlade")
  if filter == nil then
    filter = active.title and active.title:match("^(.-)%s+%-%s+FilterBlade")
  end
  if filter == nil or filter == "" then
    return
  end

  local poe = sa.window.find({
    processes = { "steam_app_2694490", "PathOfExileSteam.exe" },
  })
  if poe == nil then
    return
  end

  if not sa.window.activate(poe) then
    return
  end
  if not sa.window.wait_active(poe, 500) then
    return
  end

  sa.sleep(150)
  sa.input.key("Enter")
  sa.input.text("/reloaditemfilter " .. filter)
  sa.input.key("Enter")
end)
```

Host APIs available to imperative callbacks include `sa.sleep`,
`sa.log`/`sa.log_debug`/`sa.log_warn`, `sa.window.active`,
`sa.window.find`, `sa.window.activate`, `sa.window.wait_active`, and
`sa.input.key`/`text`/`key_down`/`key_up`/`mouse_click`. Sensitive APIs are
capability-gated and execute through Rust adapters.

## Verification

Targeted local checks:

```sh
cargo fmt --check
cargo test -p signal-auras-core controller
cargo test -p signal-auras-lua controller
cargo test --test cli_runner controller_runner
```

Full local check:

```sh
cargo clippy --all-targets -- -D warnings
cargo test
```

Nix checks when feasible:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Manual KDE Verification

Run the controller example on KDE Plasma Wayland:

```sh
just run
```

With a FilterBlade browser tab focused, press `Ctrl+S`. The browser shortcut
passes through, the callback reads the FilterBlade title, activates the PoE2
window, verifies focus, and sends `/reloaditemfilter <filter>`. Lua-visible
diagnostics are emitted as `event=lua_log` entries.
