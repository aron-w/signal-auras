# Quickstart: Lua Callback Preemption

## Target Scenario

Use a controller callback that would previously monopolize the runtime thread:

```lua
sa.hotkey({
  trigger = "F5",
  capabilities = { "global_shortcut", "timer", "synthesized_input" },
  callback = "runaway",
})

sa.callback("runaway", function()
  while true do
    -- accidental non-yielding loop
  end
  sa.input.text("must not emit after timeout")
end)
```

Expected behavior after implementation:

- The callback is interrupted by the configured execution budget.
- No text is emitted after the timeout.
- Scheduler state for `runaway` is released.
- Shutdown and timer wakeups remain serviceable.
- Diagnostics report a preempted callback disposition.

## Planned Verification Commands

```sh
cargo fmt --check
cargo test -p signal-auras-core controller
cargo test -p signal-auras-lua imperative
cargo test --test cli_runner controller_runner
cargo test
cargo clippy --all-targets -- -D warnings
XDG_CACHE_HOME=/tmp/nix-cache nix flake check
```

## Manual Desktop Check

No manual compositor check is required for the MVP. The behavior is testable through core, Lua runtime, and CLI runner harnesses without a live KDE Wayland session. Manual KDE verification may be added later as a smoke test for real shortcut delivery plus preemption diagnostics.
