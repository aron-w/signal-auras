# Implementation Plan: Robust Device Selection

**Branch**: `011-robust-device-selection` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/011-robust-device-selection/spec.md`

## Summary

Harden the unsafe evdev/uinput device-selection path so explicit selected device
paths remain strict and diagnosable, `devices = "all"` skips unusable devices
without losing eligible input, hotplug/reopen behavior is reported during the
current run, Signal Auras' own virtual output device is never observed, and
`signal-auras doctor input` explains eligibility, permissions, and preferred
stable `/dev/input/by-signal-auras/...` paths. The Lua input-provider API is
preserved.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua
5.4-compatible script configuration remains unchanged.

**Primary Dependencies**: Existing workspace crates, `libc` for evdev ioctls and
polling, existing `udev` monitor support, existing `tracing` diagnostics, and
existing `nix`/`mio` runtime dependencies from feature 007. No new dependency is
required.

**Storage**: Repository files only. No persistent device cache, daemon state,
autostart state, or discovered-device persistence.

**Testing**: `nix develop -c cargo fmt --check`,
`nix develop -c cargo clippy --all-targets -- -D warnings`,
`nix develop -c cargo test`, and `nix flake check` when the local Nix sandbox can
evaluate the project. Automated Rust tests cover simulated evdev paths, mixed
startup, reopen, noisy unsupported events, own-device exclusion, permission
failures, and doctor output.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with explicit unsafe
evdev/uinput opt-in.

**Project Type**: Rust workspace with core automation contracts, Lua
configuration parsing, Wayland/evdev adapter boundary, and CLI runner/doctor.

**Performance Goals**: Hotplug/reopen detection remains bounded by existing
udev readiness or the 1 second rescan interval; unsupported noisy events are
bounded in diagnostics and do not starve supported eligible input in simulated
tests.

**Constraints**: Preserve existing Lua syntax and current-run consent model.
Explicit selected paths must never broaden to unrelated devices. `devices =
"all"` stays an explicit current-run opt-in and persists nothing. Missing,
unreadable, permission-denied, unsupported, duplicate, and self-generated
devices must fail closed or be skipped according to selection mode with
diagnosable remediation.

**Scale/Scope**: One terminal-started runner process, local `/dev/input/event*`
devices and stable symlinks, one optional uinput output device, and the existing
live runtime event loop.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Device eligibility, selection policy, and diagnostics
  remain Rust APIs in `signal-auras-wayland`/`signal-auras-cli` with tests before
  live runner behavior.
- Wayland/Compositor Awareness: PASS. Scope remains unsafe evdev/uinput on
  NixOS/KDE Wayland; unavailable protocols and permissions remain explicit
  errors.
- Rust Safety Boundaries: PASS. File descriptor reads, ioctls, polling, udev,
  and uinput identity checks stay isolated in `crates/signal-auras-wayland`.
- Lua Extension Contract: PASS. No Lua syntax or script capability changes are
  introduced.
- NixOS Reproducibility: PASS. Verification uses the flake commands above and
  NixOS module guidance remains the permission remediation path.
- Security and Consent: PASS. Unsafe input observation remains visible
  configuration, explicit selected paths are strict, broad discovery remains
  opt-in, and no privilege escalation or persistence is added.
- TDD and Testability: PASS. Automated tests cover selection policy and doctor
  output; live compositor checks remain supplemental.
- Minimal Composition: PASS. No daemon, IPC endpoint, global registry, async
  runtime, persistent store, or broad architecture rewrite is introduced.
- No Hidden Global Behavior: PASS. Device discovery and hotplug are current-run
  only and never install hooks, services, or saved selections.
- Incremental Delivery: PASS. P1 selected devices, P2 broad discovery, P3
  hotplug/reopen, and P4 doctor diagnostics are independently reviewable.

## Project Structure

### Documentation (this feature)

```text
specs/011-robust-device-selection/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── device-selection.md
│   └── input-doctor.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-wayland/src/
├── adapter.rs
├── evdev.rs
└── uinput.rs

crates/signal-auras-cli/src/
└── runner.rs

crates/signal-auras-lua/src/
└── sandbox.rs

tests/
├── contract/
└── integration/
```

**Structure Decision**: Keep core selection and reopen behavior inside the
evdev adapter boundary, because it owns file descriptors, device identity, and
udev readiness. Keep doctor rendering in the CLI runner where the command and
permission probe already live. Preserve Lua parsing and public configuration
shape.

## Complexity Tracking

No constitution violations.
