# Implementation Plan: Lua Callback Preemption

**Branch**: `020-lua-callback-preemption` | **Date**: 2026-06-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/020-lua-callback-preemption/spec.md`

## Summary

Bound imperative Lua callback execution that does not yield to the host. The planned implementation adds a library-owned callback execution budget, extends callback dispositions with a preempted/timeout result, installs `mlua` per-coroutine instruction hooks around each active callback resume, and wires the CLI runner to release scheduler state and emit privacy-bounded diagnostics when a callback exceeds its active execution budget. Existing Lua APIs and host-yielding continuation behavior remain unchanged.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface through existing vendored `mlua` 0.10.5.

**Primary Dependencies**: Existing workspace crates plus existing `mlua` with `lua54` and `vendored` features. No new dependency is planned for the first increment.

**Storage**: Repository files only. No persistent callback queue, daemon state, IPC state, budget cache, or script execution history.

**Testing**: `cargo fmt --check`, targeted `cargo test -p signal-auras-core controller`, targeted `cargo test -p signal-auras-lua imperative`, targeted CLI contract tests for runaway callbacks, full `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `XDG_CACHE_HOME=/tmp/nix-cache nix flake check`.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland for real runtime use; most preemption behavior is testable without a live compositor through core, Lua, and CLI contract tests.

**Project Type**: Rust workspace with core automation library, Lua runtime crate, Wayland adapter crate, CLI runner, and contract/integration tests.

**Performance Goals**: A non-yielding callback is interrupted within its configured budget plus hook granularity tolerance in automated tests; normal callback dispatch still targets p95 <= 20 ms and p99 <= 50 ms before Lua execution; repeated trigger stress keeps per-trigger pending work bounded with explicit accepted, skipped, denied, dropped, cancelled, failed, completed, slow, or preempted disposition.

**Constraints**: Preserve existing Lua-facing APIs, current-run capability consent, rooted imports, sandbox policy, non-blocking `sa.sleep`, privacy-bounded diagnostics, Nix reproducibility, and no hidden background daemon, async runtime, global registry, or persistent state.

**Scale/Scope**: One terminal-started runner process, one embedded Lua controller runtime, one active callback resume on the runtime thread at a time, bounded continuation queue, and existing global shortcut/evdev/uinput/portal runtime paths.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Budget policy, preempted disposition, scheduler state release, and diagnostic contracts are planned as Rust library behavior before CLI wiring.
- Wayland/Compositor Awareness: PASS. The feature changes Lua callback execution semantics only; compositor capabilities continue to fail closed through existing KDE/Wayland adapters.
- Rust Safety Boundaries: PASS. Lua receives no OS handles. The execution hook is installed by Rust around callback resume and all OS-facing work remains in host APIs and adapters.
- Lua Extension Contract: PASS. Existing Lua functions remain stable; preemption is a runtime safety boundary for callbacks that exceed budget.
- NixOS Reproducibility: PASS. No new dependency is planned; verification uses cargo and flake commands already supported by the repository.
- Security and Consent: PASS. Preemption reduces script risk and preserves explicit current-run capabilities for input, process/window metadata, timers, and synthesized output.
- TDD and Testability: PASS. Core scheduler, Lua runtime, and CLI contract tests are planned before implementation, including infinite-loop and post-resume runaway callbacks.
- Minimal Composition: PASS. `mlua` instruction hooks are the smallest available enforcement mechanism; worker isolation is deferred unless hooks cannot satisfy the contract.
- No Hidden Global Behavior: PASS. No daemon, IPC endpoint, autostart, or hidden persistence is introduced.
- Incremental Delivery: PASS. The MVP is independently useful: stop runaway imperative callbacks while preserving yielding callback behavior.

## Project Structure

### Documentation (this feature)

```text
specs/020-lua-callback-preemption/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── lua-callback-preemption.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── controller.rs
└── lib.rs

crates/signal-auras-lua/src/
├── runtime.rs
└── lib.rs

crates/signal-auras-cli/src/
├── runner.rs
└── runner/controller.rs

tests/contract/
├── cli_runner.rs
└── lua_api.rs

tests/integration/
└── runner_flow.rs
```

**Structure Decision**: Keep budget policy and callback disposition semantics in `signal-auras-core::controller`; keep Lua hook installation, timeout detection, and coroutine error mapping in `signal-auras-lua::runtime`; keep runner handling limited to passing task budgets, releasing scheduler state, recording stats, and logging privacy-bounded diagnostics.

## Complexity Tracking

No constitution violations.
