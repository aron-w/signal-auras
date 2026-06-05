# Implementation Plan: Runtime Shutdown Reliability

**Branch**: `012-runtime-shutdown-reliability` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/012-runtime-shutdown-reliability/spec.md`

## Summary

Harden runtime shutdown and startup-failure cleanup so SIGINT and SIGTERM are blocked before helper threads spawn, routed through the runtime signal fd, and converted into prompt main-loop shutdown. Cleanup remains current-run only and releases virtual input devices, evdev observation providers, evdev grabs, portal/screencast sessions, overlays, KDE bridge scripts, callbacks, and shortcut registrations with diagnosable summaries.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake.

**Primary Dependencies**: Existing `mio`, `nix` signal/signalfd support, `libc` fd polling, `udev`, `tracing`, KDE bridge, evdev, and uinput modules. No new dependency is expected.

**Storage**: Repository files only. No persistent shutdown state, daemon state, IPC state, or signal registry.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated tests cover SIGINT/SIGTERM routing, signal-mask startup ordering, helper-thread inheritance, prompt wakeups, no-new-work-after-shutdown, startup failure after partial input-provider acquisition, and cleanup idempotency.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with explicit current-run input observation and synthesized input consent.

**Project Type**: Rust workspace with core runtime stats, Wayland/evdev/KDE adapters, and CLI runner.

**Performance Goals**: Shutdown wakeups should reach the main loop within the existing event-loop wake target without waiting for unrelated input or callbacks.

**Constraints**: Preserve Lua APIs, current-run consent, fail-closed permission behavior, no daemon/autostart/hidden IPC, and no persistent resource ownership.

**Scale/Scope**: One terminal-started runner process with runtime listener/helper threads, signal fd, wake fd, evdev/uinput resources, and KDE bridge resources.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-check after Phase 1 design: Passed for planned scope.*

- Library-First: PASS. Signal guard and shutdown state are testable Rust behavior before CLI wiring.
- Wayland/Compositor Awareness: PASS. KDE/evdev/uinput cleanup remains explicit and current-run scoped.
- Rust Safety Boundaries: PASS. Signal masks, signalfd, fds, grabs, and virtual input cleanup stay behind narrow modules with tests.
- Lua Extension Contract: PASS. No Lua syntax or capability change.
- NixOS Reproducibility: PASS. Verification uses the project flake commands.
- Security and Consent: PASS. Shutdown only revokes current-run resources and adds no ambient access.
- TDD and Testability: PASS. Signal routing, wakeups, and cleanup behavior require automated tests before implementation.
- Minimal Composition: PASS. No daemon, async runtime, persistent store, or global registry is introduced.
- No Hidden Global Behavior: PASS. No hooks, autostart, or persistent signal handling side effects are added.
- Incremental Delivery: PASS. Signal routing, helper-thread masks, and cleanup wakeups are independently reviewable.

## Project Structure

### Documentation (this feature)

```text
specs/012-runtime-shutdown-reliability/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-wayland/src/
├── event_loop.rs
├── kde_bridge.rs
├── evdev.rs
└── uinput.rs

crates/signal-auras-cli/src/
└── runner.rs

crates/signal-auras-core/src/
└── stats.rs

tests/
├── contract/
└── integration/
```

**Structure Decision**: Keep low-level signal fd and wake fd ownership in `signal-auras-wayland::event_loop`, keep listener/helper thread startup ordering in the adapters that spawn them, and keep CLI changes limited to runtime lifecycle ordering, no-new-work-after-shutdown, and final summaries.

## Complexity Tracking

No constitution violations.
