# Implementation Plan: Lua Controller Runtime

**Branch**: `016-lua-controller-runtime` | **Date**: 2026-06-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/016-lua-controller-runtime/spec.md`

## Summary

Introduce Lua as a controller surface while preserving Rust as the trusted automation core. This increment defines standalone Rust contracts for controller registration validation, bounded Lua callback scheduling, Rust-backed output batching, and capability enforcement, adds an embedded Lua 5.4 coroutine runtime for imperative callbacks, represents `sa.sleep` as scheduled continuation work, wires the live controller runner to Rust-owned window, timer, logging, and synthesized-input host APIs, unifies controller/runtime sandbox denied globals without changing the existing declarative Lua configuration loader, and makes imperative runtime activation consume the same rooted controller source tree as registration/program validation.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains the user-facing extension target.

**Primary Dependencies**: Existing workspace crates plus `mlua` with vendored Lua 5.4 support for the embedded controller runtime.

**Storage**: Repository files only. No persistent callback queue, registration cache, daemon state, IPC state, output cache, or script module cache across runs.

**Testing**: `cargo fmt --check`, targeted `cargo test -p signal-auras-core controller`, targeted `cargo test -p signal-auras-lua controller`, targeted CLI and uinput contract tests, full `cargo test`, `cargo clippy --all-targets -- -D warnings`, and Nix verification commands `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing explicit current-run consent for global shortcuts, evdev observation/grab, process metadata, and synthesized input.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, CLI runner, and contract/integration tests.

**Performance Goals**: Registration validation is in-memory and side-effect free. Simulated callback scheduling remains bounded with p95 <= 20 ms and p99 <= 50 ms before Lua execution; per-trigger pending work remains bounded to one active/pending task. Stress tests should record explicit accepted, skipped, denied, dropped, cancelled, failed, completed, or slow disposition.

**Constraints**: Preserve existing declarative Lua API, fail closed on denied or unprobed capabilities, keep all OS-facing work in Rust adapters, deny ambient Lua filesystem/shell/network/debug/package access through one shared policy, avoid hidden global behavior, and do not introduce a daemon, async runtime, persistent state, or ambient dynamic registration.

**Scale/Scope**: One terminal-started runner process, one main controller script plus local modules rooted at that script directory, current-run registrations, bounded callback queue, bounded output batch, embedded Lua callbacks loaded from the resolved source tree, KWin-backed active-window lookup/activation on KDE Plasma Wayland, and the existing Wayland adapter capability model.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. `signal-auras-core::controller` defines registration, callback scheduling, output batching, and capability contracts before CLI or adapter activation.
- Wayland/Compositor Awareness: PASS. Controller registrations only declare requirements; KDE/Wayland global shortcut, evdev, focus, and synthesized input availability still fail closed through the existing capability report.
- Rust Safety Boundaries: PASS. Lua receives no direct OS handle. Global input, focus metadata, output emission, timers, wake fds, and cleanup remain Rust-owned.
- Lua Extension Contract: PASS. Existing declarative scripts are unchanged. New controller APIs are separate and versionable through the controller loader contract.
- NixOS Reproducibility: PASS. No new dependency is required for this increment; verification uses existing flake commands.
- Security and Consent: PASS. Rooted imports, ambient API denial, explicit capability checks, bounded queues, and no runtime activation during registration are required.
- TDD and Testability: PASS. Core scheduler/output tests, Lua controller loader/runtime tests, CLI runner tests, KWin adapter tests, and uinput text mapping tests cover the new behavior.
- Minimal Composition: PASS. No daemon, IPC endpoint, async runtime, global registry, or persistent store is introduced.
- No Hidden Global Behavior: PASS. Controller loading only collects definitions; providers are installed only after validation and explicit runtime activation.
- Incremental Delivery: PASS. Registration, scheduler/output contracts, live callback execution, and the PoE2 FilterBlade relay are independently testable increments.

## Project Structure

### Documentation (this feature)

```text
specs/016-lua-controller-runtime/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── lua-controller-api.md
│   └── rust-controller-library.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── controller.rs
└── lib.rs

crates/signal-auras-lua/src/
├── sandbox.rs
├── runtime.rs
└── lib.rs

crates/signal-auras-cli/src/
└── runner.rs

crates/signal-auras-wayland/src/
├── adapter.rs
├── capability.rs
├── kde_bridge.rs
└── uinput.rs

tests/contract/
├── cli_runner.rs
├── lua_api.rs
└── rust_library.rs
```

**Structure Decision**: Keep trusted controller semantics in `signal-auras-core::controller`; keep restricted source/module loading, runtime source-tree resolution, and static compatibility parsing in `signal-auras-lua::sandbox`; keep the shared denied-global policy in `signal-auras-lua::sandbox_policy`; keep embedded coroutine execution in `signal-auras-lua::runtime`; keep all OS-facing sleep, window, logging, and input effects in the CLI runner and Wayland adapter.

## Complexity Tracking

No constitution violations.
