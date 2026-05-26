# Implementation Plan: Input Motion Performance and Consistency

**Branch**: `006-input-performance` | **Date**: 2026-05-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-input-performance/spec.md`

## Summary

Improve the existing unsafe evdev/uinput motion runtime so supported keyboard, pointer button, and wheel events are observed with low latency, repeat cancellation wins over pending repeat ticks, `devices = "all"` handles hotplug during the current run, and verbose diagnostics make latency and cancellation behavior explainable. The implementation keeps the Lua motion API unchanged and focuses on Rust library behavior, adapter polling, CLI scheduling, tests, and documentation.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates plus existing `libc` use for evdev/uinput readiness polling and file descriptor safety boundaries. No new runtime dependency is required.

**Storage**: Repository files only. No persistent runtime state, daemon state, or device cache.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when the Nix sandbox can evaluate the project. Automated tests cover simulated evdev input, repeat cancellation, hotplug, and profiling.

**Target Platform**: NixOS and KDE Plasma Wayland with explicit unsafe evdev/uinput opt-in.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Simulated mixed-device input dispatch p95 <= 20 ms and p99 <= 50 ms; device hotplug detection <= 1 second for `devices = "all"`; no repeat macro after a processed cancellation release.

**Constraints**: No new Lua motion syntax, no hidden global behavior, no daemon/autostart, no ambient Lua capabilities, and no expansion of unsafe input scope. Input observation, grabs, and uinput output remain explicit current-run permissions.

**Scale/Scope**: One terminal-started runner process, a small set of local `/dev/input/event*` devices, and existing motion examples. This feature optimizes consistency rather than adding broader automation primitives.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Repeat timing, cancellation, fairness, and metrics are represented as Rust behavior with tests before CLI integration.
- Wayland/Compositor Awareness: PASS. KDE Wayland remains the target; unsafe evdev/uinput assumptions and unavailable permission behavior are explicit.
- Rust Safety Boundaries: PASS. Low-level polling and device fd operations stay isolated in the wayland adapter with documented invariants.
- Lua Extension Contract: PASS. The Lua API is unchanged and no new script capability is added.
- NixOS Reproducibility: PASS. Verification uses the project flake commands.
- Security and Consent: PASS. Unsafe evdev/uinput remains opt-in, current-run only, and fail-closed.
- Testable Automation Behavior: PASS. Simulated input and repeat tests cover timing-sensitive behavior; manual KDE verification remains supplemental.
- Minimal Composition: PASS. No daemon, async runtime, global registry, or persistent state is introduced.
- No Hidden Global Behavior: PASS. No new hooks, services, autostart, IPC, or persistence.
- Incremental Delivery: PASS. Reliability is delivered as an independently useful hardening increment over existing motions.

## Project Structure

### Documentation (this feature)

```text
specs/006-input-performance/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── runtime-scheduler.md
│   └── unsafe-evdev-provider.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── macro_plan.rs
├── motion.rs
└── stats.rs

crates/signal-auras-wayland/src/
├── adapter.rs
└── evdev.rs

crates/signal-auras-cli/src/runner.rs

tests/integration/runner_flow.rs
tests/contract/lua_api.rs
README.md
```

**Structure Decision**: Keep timing and repeat semantics testable in existing Rust modules. Keep file descriptor readiness, rescan, and device diagnostics inside the unsafe evdev adapter boundary. Keep CLI changes limited to the live runner scheduler and verbose logs.

## Complexity Tracking

No constitution violations.
