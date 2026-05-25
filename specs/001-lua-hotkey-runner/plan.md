# Implementation Plan: Lua Hotkey Runner

**Branch**: `001-lua-hotkey-runner` | **Date**: 2026-05-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-lua-hotkey-runner/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Build the first Signal Auras runner as a terminal-started CLI that loads exactly one sandboxed Lua automation file, validates hotkey and macro configuration, prompts for explicit current-run scope when the script omits it, registers process-scoped or explicit-global hotkeys through a Wayland capability adapter, executes allowed macro actions in order, and prints auditable runtime stats on Ctrl-C.

The technical approach is library-first: pure Rust crates own config validation, scope matching, macro planning, stats accounting, consent decisions, and diagnosable errors. Lua, CLI, and Wayland/compositor code are integration layers over those contracts. Unsupported compositor protocols, missing permissions, unavailable active-process metadata, and denied synthesized-input capabilities fail before hotkey registration or macro execution instead of silently degrading.

## Technical Context

**Language/Version**: Rust stable toolchain from the project flake; Lua 5.4-compatible script surface exposed through a Rust Lua embedding crate.

**Primary Dependencies**: New Cargo workspace; planned crates include `mlua` for embedded Lua sandboxing, `thiserror` for typed errors, `tracing`/`tracing-subscriber` for structured terminal logs, `clap` for CLI argument validation, `ctrlc` or `tokio` signal handling for Ctrl-C, and Wayland/portal crates selected behind adapter modules (`wayland-client`, `wayland-protocols`, `ashpd` or equivalent) as implementation tasks refine compositor support. All dependencies must be represented in `flake.nix`/Cargo metadata or documented as unavailable.

**Storage**: N/A for v1. Scope prompt selections, runtime stats, and registrations are current-run memory only. No persistent config, daemon state, IPC state, or autostart state.

**Testing**: TDD with `cargo test` through `nix develop -c cargo test`. Library tests for script validation, scope decisions, macro action ordering, stats, consent flow decisions, and error modeling. Lua sandbox tests for denied ambient APIs. CLI contract tests for argument validation and prompt behavior. Adapter contract tests use mocks; real compositor behavior uses documented manual verification until an automated Wayland harness exists.

**Target Platform**: NixOS Wayland session started from a terminal. X11 is out of scope. Compositor-specific support must be detected at startup/runtime and produce diagnosable errors when required protocols or permissions are absent.

**Project Type**: Rust workspace with library-first crates plus CLI entrypoint, Lua binding crate, and narrow Wayland/portal adapter crate.

**Performance Goals**: Startup validation and registration feedback visible within 500 ms after Lua file load on a normal development machine, excluding user permission prompts. Scope checks and macro planning are deterministic in-process operations. Macro actions execute in declared order with delay durations honored within the scheduler and no overlapping execution for the same hotkey in v1.

**Constraints**: Explicit consent for scope/global behavior; no hidden global defaults; Lua has no ambient filesystem, network, process, shell, environment, compositor, or global-input access; Wayland limitations fail explicitly; Rust library APIs precede integration layers; NixOS verification commands are required; no daemon, persistence, IPC, autostart, or background service in v1.

**Scale/Scope**: Single terminal runner process, one Lua file per invocation, one effective scope per run, multiple hotkeys per file, process-name matching by user-visible executable/process name, macro actions limited to key, text, and delay.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. `signal-auras-core` owns config, scope, macro planning, stats, consent, and errors before CLI/Lua/Wayland integration.
- Wayland/Compositor Awareness: PASS. Wayland and portal behavior is isolated in adapter contracts; unsupported protocols, missing permissions, and unavailable active-process metadata fail diagnostically.
- Rust Safety Boundaries: PASS. Process inspection, global hotkey registration, synthesized input, and protocol calls are confined to `signal-auras-wayland` adapters with documented invariants and mockable contracts.
- Lua Extension Contract: PASS. Lua scripts use a versioned macro-building API and receive no ambient host capabilities.
- NixOS Reproducibility: PASS. Verification uses `nix develop -c cargo test`, `nix develop -c cargo fmt --check`, and `nix develop -c cargo clippy --all-targets -- -D warnings`; flake updates are planned for Rust tooling and native dependencies.
- Security and Consent: PASS. Scope decisions are explicit, visible, current-run only, and revocable by Ctrl-C; global behavior requires explicit terminal selection.
- TDD and Testability: PASS. Failing tests are planned first for library behavior and sandbox constraints; manual compositor checks are documented for non-automated Wayland interactions.
- Minimal Composition: PASS. No daemon, async runtime mandate, persistent store, plugin system, or global registry is introduced. If async is needed by portal bindings, it remains adapter-local and justified by that dependency.
- No Hidden Global Behavior: PASS. No registration occurs without validated config and effective scope; absent scope prompts instead of defaulting global.
- Incremental Delivery: PASS. The P1 path can land as scoped validation/registration with mocks before prompt, trigger, and shutdown stories.

## Project Structure

### Documentation (this feature)

```text
specs/001-lua-hotkey-runner/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── cli.md
│   ├── lua-api.md
│   └── rust-library.md
└── tasks.md             # Created by /speckit-tasks, not /speckit-plan
```

### Source Code (repository root)

```text
Cargo.toml
crates/
├── signal-auras-core/
│   ├── Cargo.toml
│   └── src/
│       ├── config.rs
│       ├── consent.rs
│       ├── error.rs
│       ├── hotkey.rs
│       ├── lib.rs
│       ├── macro_plan.rs
│       ├── scope.rs
│       └── stats.rs
├── signal-auras-lua/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── sandbox.rs
├── signal-auras-wayland/
│   ├── Cargo.toml
│   └── src/
│       ├── adapter.rs
│       ├── diagnostics.rs
│       ├── lib.rs
│       └── portal.rs
└── signal-auras-cli/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── prompt.rs
        └── runner.rs

tests/
├── contract/
│   ├── cli_runner.rs
│   ├── lua_api.rs
│   └── rust_library.rs
├── integration/
│   └── runner_flow.rs
└── compositor/
    └── manual-wayland-verification.md
```

**Structure Decision**: Use a new Cargo workspace because the repository does not yet contain source code. Split core semantics, Lua embedding, Wayland adapters, and CLI orchestration into separate crates so constitution-sensitive behavior is testable without a compositor and each safety boundary has a narrow contract.

## Complexity Tracking

No constitution violations are planned.
