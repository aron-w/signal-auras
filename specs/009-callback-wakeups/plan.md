# Implementation Plan: Callback Wakeups

**Branch**: `009-callback-wakeups` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-callback-wakeups/spec.md`

## Summary

Make compositor shortcut callbacks a first-class wake source for the live runner
so KDE callback delivery is prompt even when no physical input, repeat timer, or
shutdown signal arrives. The implementation keeps the Lua API unchanged, adds a
bounded callback queue and wake fd behind the KDE bridge boundary, records
privacy-bounded callback latency/disposition metrics, and updates the live
runtime loop to sleep until real callback, input, timer, hotplug, or shutdown
readiness.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua
5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates plus existing `mio`, `nix`,
`udev`, `tracing`, `tracing-subscriber`, `libc`, and `zbus` runtime dependencies.
No new dependency is required.

**Storage**: Repository files only. No persistent callback queue, daemon state,
IPC state, or shortcut cache.

**Testing**: `nix develop -c cargo fmt --check`,
`nix develop -c cargo clippy --all-targets -- -D warnings`,
`nix develop -c cargo test`, and `nix flake check` when feasible. Automated
coverage targets callback wake fd readiness, bounded queue disposition,
callback latency stats, idle wait timeout behavior, mixed callback/input/repeat
processing, shutdown no-start behavior, unavailable callback support, and
diagnostic fields.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing explicit
current-run shortcut, process metadata, and synthesized input consent.

**Project Type**: Rust workspace with core automation library, Lua validation
crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Simulated callback-to-dispatch p95 <= 20 ms and p99 <=
50 ms; at least 1,000 simulated callback events have explicit accepted,
dispatched, denied, ignored, or dropped disposition; idle loop avoids continuous
short polling when no callback, input, repeat, macro, hotplug, or shutdown work
is pending.

**Constraints**: Preserve Lua hotkey/motion syntax, existing consent gates,
fail-closed callback registration behavior, process-awareness checks, privacy
bounded diagnostics, current-run cleanup, Nix reproducibility, and no hidden
global behavior.

**Scale/Scope**: One terminal-started runner process, current-run KDE KWin
shortcut bridge callbacks, existing evdev/uinput/portal runtime paths, and
normal desktop callback bursts with a documented bounded queue limit.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Callback queue disposition and latency counters are
  tested Rust behavior before CLI/runtime wiring.
- Wayland/Compositor Awareness: PASS. KDE Plasma Wayland KWin callbacks remain
  the real desktop target; missing KWin/KGlobalAccel/D-Bus support fails closed.
- Rust Safety Boundaries: PASS. The wake fd and D-Bus listener stay isolated in
  the Wayland adapter boundary with tests and explicit cleanup.
- Lua Extension Contract: PASS. No Lua API or capability syntax changes.
- NixOS Reproducibility: PASS. Verification uses the project flake commands and
  adds no new dependency.
- Security and Consent: PASS. Callback-triggered macros reuse existing current
  run shortcut registration, process metadata, and macro execution consent.
- TDD and Testability: PASS. Queue, wake fd, stats, and runner behavior are
  covered with automated tests; manual KDE verification remains supplemental.
- Minimal Composition: PASS. No daemon, persistent queue, background service,
  async runtime, global registry, or hidden IPC is introduced.
- No Hidden Global Behavior: PASS. Shortcuts remain explicitly configured,
  current-run scoped, and cleaned up on shutdown/error.
- Incremental Delivery: PASS. Callback wakeup reliability is a separable runtime
  hardening increment over the existing KDE bridge.

## Project Structure

### Documentation (this feature)

```text
specs/009-callback-wakeups/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── callback-wakeup.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── lib.rs
└── stats.rs

crates/signal-auras-wayland/src/
├── adapter.rs
├── event_loop.rs
├── kde_bridge.rs
└── lib.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/contract/
├── cli_runner.rs
└── rust_library.rs

tests/integration/
└── runner_flow.rs
```

**Structure Decision**: Keep callback metrics in `signal-auras-core::stats`,
keep wake fd and callback queue ownership in `signal-auras-wayland`, and keep
CLI changes limited to runtime loop wake ordering, diagnostics, and shutdown
no-start behavior.

## Complexity Tracking

No constitution violations.
