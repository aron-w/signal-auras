# Implementation Plan: Wayland Compositor Adapter

**Branch**: `002-wayland-compositor-adapter` | **Date**: 2026-05-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-wayland-compositor-adapter/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Complete the first real desktop-wide adapter for KDE Plasma Wayland on NixOS. The current skeleton already models capability probing, fail-closed diagnostics, registration lifecycles, active-process contexts, synthesized-input requests, runtime stats, and CLI composition. This plan turns that boundary into a KDE provider by adding explicit KDE session detection, current-run D-Bus/KWin integration for global shortcut delivery and focused-window metadata, xdg-desktop-portal RemoteDesktop input synthesis, and manual KDE verification that proves all three user stories on a real Plasma Wayland session.

## Technical Context

**Language/Version**: Rust stable toolchain from the project flake; existing Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates plus Rust D-Bus/portal dependencies in `signal-auras-wayland`: `zbus` for KDE session D-Bus integration and `ashpd` for xdg-desktop-portal RemoteDesktop input. Keep existing `wayland`, `wayland-protocols`, `dbus`, and `xdg-desktop-portal` Nix packages, and add KDE Plasma verification tools/packages only when needed by implementation.

**Storage**: N/A for persistent storage. The KDE provider may create current-run in-memory registrations, D-Bus object paths, portal sessions, and optional temporary KWin script identifiers; all must be revoked or unloaded on shutdown. No autostart, persistent grant store, daemon state, or config store is introduced.

**Testing**: TDD with `nix develop -c cargo test`, `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, and `nix flake check`. Add automated tests for KDE provider selection, capability mapping, D-Bus/portal error mapping, active-process snapshot conversion, input request sequencing, and cleanup state. Live KDE Plasma Wayland behavior remains manual until a repeatable nested Plasma harness exists.

**Target Platform**: NixOS KDE Plasma Wayland session. Non-KDE sessions, X11 sessions, missing KWin services, missing xdg-desktop-portal-kde support, denied permissions, and unsupported KDE protocol paths fail explicitly before activation.

**Project Type**: Existing Rust workspace with library-first core, Wayland/KDE adapter crate, Lua binding crate, CLI runner, contract tests, integration tests, and manual compositor verification.

**Performance Goals**: KDE capability probe and registration diagnostics appear within 2 seconds after user permission decisions complete. Shortcut event handling evaluates active-process context before macro execution. Shutdown unregisters shortcuts, unloads current-run KDE bridge state, closes portal sessions, cancels pending input, and stops accepting events within 1 second.

**Constraints**: Explicit current-run consent for global shortcuts, active-process metadata use, and synthesized input; no hidden KDE global shortcuts, KWin scripts, portal sessions, background service, autostart, persistent D-Bus activation, or broad Lua access. Any D-Bus object, KWin script, portal session, protocol binding, or process/window metadata handling is isolated in `signal-auras-wayland` with documented invariants and tests.

**Scale/Scope**: Single terminal runner process, one Lua file per invocation, multiple hotkeys per run, process-name matching from active KDE window metadata, macro actions limited to key, text, and delay. Pixel checks, image checks, broader window queries, persistent grants, X11 adapters, and non-KDE providers remain out of scope.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Core capability decisions, registration lifecycle states, active-process matching, macro sequencing, stats, and diagnosable errors remain standalone Rust library behavior; KDE code is an adapter implementation.
- Wayland/Compositor Awareness: PASS. KDE Plasma Wayland is the first provider target, with explicit D-Bus, KWin, portal, permission, unsupported-session, and cleanup behavior documented.
- Rust Safety Boundaries: PASS. D-Bus object ownership, portal session lifetime, KWin script bridge state, and any process/window metadata handling stay behind `signal-auras-wayland` modules with tests and invariants.
- Lua Extension Contract: PASS. Lua remains configuration-only; scripts receive no raw KDE D-Bus, active-window, or input-injection capability.
- NixOS Reproducibility: PASS. Verification commands use the flake; new Rust and native KDE/portal dependencies must be represented in `Cargo.toml`, `Cargo.lock`, and `flake.nix` or documented as unavailable.
- Security and Consent: PASS. Global shortcuts, metadata reads, synthesized input, KWin bridge installation, and portal sessions require visible current-run intent and revocation on shutdown.
- TDD and Testability: PASS. Pure behavior and adapter contracts are automated; real KDE desktop behavior has exact manual verification until a nested Plasma harness is added.
- Minimal Composition: PASS. No daemon, persistent store, plugin system, autostart, or hidden background process is introduced. A current-run D-Bus/KWin bridge is allowed only as a visible side-effect adapter required for KDE integration and must be removed on shutdown.
- No Hidden Global Behavior: PASS. KDE registrations, bridge state, portal sessions, ignored events, emitted input, and cleanup are printed or counted.
- Incremental Delivery: PASS. US1 global shortcut delivery, US2 active-process matching, and US3 synthesized input remain independently testable, but the feature is not complete until all three work on KDE Plasma Wayland.

## Project Structure

### Documentation (this feature)

```text
specs/002-wayland-compositor-adapter/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── adapter-contract.md
│   ├── cli.md
│   └── rust-library.md
└── tasks.md
```

### Source Code (repository root)

```text
Cargo.toml
crates/
├── signal-auras-core/
│   └── src/
│       ├── config.rs
│       ├── consent.rs
│       ├── error.rs
│       ├── hotkey.rs
│       ├── macro_plan.rs
│       ├── scope.rs
│       └── stats.rs
├── signal-auras-wayland/
│   └── src/
│       ├── adapter.rs          # provider selection and trait composition
│       ├── capability.rs       # session and capability probing
│       ├── diagnostics.rs      # KDE/portal error mapping
│       ├── input.rs            # synthesized input adapter boundary
│       ├── kde.rs              # KDE Plasma provider facade
│       ├── kde_bridge.rs       # current-run D-Bus/KWin bridge boundary
│       ├── process.rs          # active-window/process snapshot mapping
│       ├── portal.rs           # xdg-desktop-portal RemoteDesktop integration
│       └── shortcut.rs         # KDE shortcut registration/event delivery
├── signal-auras-lua/
│   └── src/
│       ├── lib.rs
│       └── sandbox.rs
└── signal-auras-cli/
    └── src/
        ├── lib.rs
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

**Structure Decision**: Continue the existing four-crate workspace. Add KDE-specific implementation modules only inside `signal-auras-wayland`; keep core, Lua, and CLI layers provider-agnostic except for user-visible diagnostics and selection of the real adapter.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Current-run D-Bus/KWin bridge | KDE Plasma Wayland does not expose all needed shortcut event and focused-window behavior as pure in-process Rust library calls; the adapter may need a current-run D-Bus object and temporary KWin bridge state. | A mock-only adapter does not satisfy the feature, and persistent daemon/autostart behavior would violate no-hidden-global behavior. |
| Manual KDE compositor verification | Real shortcut capture, focused-window metadata, portal permission prompts, and input injection require an interactive Plasma Wayland session. | CI-only fake adapter tests cannot prove desktop-wide behavior; a full nested Plasma harness is future work outside this feature. |

