# Implementation Plan: Interactive Device Cache

**Branch**: `022-interactive-device-cache` | **Date**: 2026-06-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/022-interactive-device-cache/spec.md`

## Summary

Add mandatory runtime-scoped interactive evdev device selection for Lua scripts
that set `devices = "interactive"`. Startup derives a cache file from the
canonical main Lua path under `$XDG_RUNTIME_DIR/signal-auras/input-devices/`,
validates cached device identities and permissions, resolves accepted cache
entries into strict selected evdev paths, otherwise prompts in the terminal,
optionally requests selected-device ACLs, rewrites the runtime cache, and starts.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua
5.4-compatible script surface.

**Primary Dependencies**: Existing workspace crates only. Use std filesystem,
process, and terminal I/O; existing `libc`/evdev helpers remain in
`signal-auras-wayland`.

**Storage**: Volatile runtime files under
`$XDG_RUNTIME_DIR/signal-auras/input-devices/`. No persistent config store,
daemon state, IPC state, or cross-session cache.

**Testing**: `cargo fmt --check`, targeted cargo tests for core/lua/cli/wayland,
`cargo test`, `cargo clippy --all-targets -- -D warnings`, and Nix commands
`nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets
-- -D warnings`, `nix develop -c cargo test`, and `nix flake check` where
available.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with explicit unsafe
evdev/uinput opt-in.

**Project Type**: Rust workspace with core automation contracts, Lua parsing,
Wayland/evdev adapter boundary, and CLI runner/doctor.

**Performance Goals**: Valid cache checks are bounded filesystem/sysfs probes
done once during startup. The live input event loop remains unchanged after
interactive resolution.

**Constraints**: Cache must be mandatory for interactive mode but volatile.
Missing or stale cache in non-interactive startup fails closed. Permission
repair is user-confirmed and selected-device scoped. Lua receives no cache,
device, or helper handles. `devices = "all"` behavior stays separate.

**Scale/Scope**: One terminal-started runner, one canonical main Lua script,
small local selected device set, optional `/dev/uinput`, and existing evdev
hotplug/reopen behavior after startup.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Interactive selection state, cache keys, validation, and
  Lua provider shape are Rust-owned contracts before CLI wiring.
- Wayland/Compositor Awareness: PASS. KDE Plasma portal limitations are
  documented; real input observation remains unsafe evdev/uinput.
- Rust Safety Boundaries: PASS. Sysfs/device identity probing and evdev opening
  remain in Rust; sudo ACL invocation is isolated behind an explicit CLI helper.
- Lua Extension Contract: PASS. Lua only declares `devices = "interactive"`;
  it receives no cache or permission authority.
- NixOS Reproducibility: PASS. No new dependencies; verification uses existing
  flake commands.
- Security and Consent: PASS. Prompt, cache, and ACL repair are explicit,
  current-user scoped, diagnosable, and revocable.
- TDD and Testability: PASS. Parser, cache, validation, prompt decisions,
  doctor output, and example loading are covered with automated tests.
- Minimal Composition: PASS. No daemon, IPC, persistent store, TUI dependency,
  or global registry is introduced.
- No Hidden Global Behavior: PASS. Cache exists only for scripts that visibly
  request interactive device selection and is stored in the runtime directory.
- Incremental Delivery: PASS. Valid cache, first-run prompt, permission repair,
  and doctor diagnostics are independently testable slices.

## Project Structure

### Documentation (this feature)

```text
specs/022-interactive-device-cache/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── lua-input-provider.md
│   ├── runtime-device-cache.md
│   └── input-doctor.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── config.rs
└── lib.rs

crates/signal-auras-lua/src/
├── sandbox.rs
└── lib.rs

crates/signal-auras-wayland/src/
├── evdev.rs
└── lib.rs

crates/signal-auras-cli/src/
├── input_cache.rs
├── prompt.rs
├── runner.rs
└── lib.rs

tests/contract/
├── cli_runner.rs
└── lua_api.rs

examples/
└── poe2.lua
```

**Structure Decision**: Keep provider shape and Lua parsing in existing
core/Lua modules, keep hardware identity probing beside evdev code, and keep
runtime cache/prompt/ACL orchestration in the CLI because it owns startup I/O.
The Wayland adapter still receives a normal explicit selected-device provider.

## Complexity Tracking

No constitution violations.
