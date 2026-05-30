# Quickstart: Robust Device Selection

## Automated Verification

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

If Nix cannot evaluate in the local sandbox, run the equivalent `cargo` commands
directly and record the Nix failure.

## Manual Diagnostic Smoke Test

1. Create a Lua file that configures an evdev input provider with selected
   `/dev/input/by-signal-auras/...` paths.
2. Run `cargo run -p signal-auras-cli -- doctor input ./path/to/script.lua`.
3. Confirm selected paths are reported individually, `/dev/uinput` is checked
   when configured, and remediation mentions `programs.signal-auras.unsafeInput`.
4. Change the Lua file to `devices = "all"` and confirm the report warns that
   broad discovery is current-run only and recommends selected stable paths for
   daily use.

## Live Runner Smoke Test

Run only in an explicit high-trust local Wayland session:

```sh
cargo run -p signal-auras-cli -- run --verbose ./examples/input-motions.lua
```

Confirm unreadable or unsupported event devices are skipped with diagnostics,
eligible selected devices still deliver supported input, Ctrl-C cleanup runs,
and no discovered device state is persisted after process exit.
