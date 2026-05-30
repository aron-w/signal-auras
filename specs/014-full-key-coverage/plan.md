# Implementation Plan: Full Keyboard Key Coverage

**Branch**: `014-full-key-coverage` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/014-full-key-coverage/spec.md`

## Summary

Expand Signal Auras key handling from small ad hoc lists to a shared Rust key identity model that covers standard Linux evdev keyboard-like keys for triggers and macro output. The implementation introduces a generated upstream Linux key table in the core crate, canonical names and aliases for Lua compatibility, evdev raw-code decoding, uinput output mapping, and an explicit current-run key discovery doctor path. Existing Lua shapes stay compatible, and unsupported provider/backend cases fail closed with diagnostics.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates plus a generated key table derived from upstream Linux `input-event-codes.h` data and committed into `signal-auras-core`. No new runtime dependency is planned unless implementation proves an existing maintained Linux input keycode library is already available through the flake without widening runtime scope.

**Storage**: Repository files only. No persistent key discovery cache, device state, alias store, daemon state, or IPC state.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated coverage targets key parsing/canonicalization, alias compatibility, duplicate detection, evdev decoding, uinput output mapping, unsupported backend/provider diagnostics, doctor discovery reports, permission denial, no persistence, and existing Lua compatibility.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing explicit current-run unsafe evdev/uinput opt-in for physical motion observation and macro output.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Key parsing and raw-code lookup are in-memory table lookups with no device I/O. Runtime dispatch p95/p99 goals from the existing input-performance and runtime-event-loop plans remain unchanged. Discovery emits bounded per-event diagnostics and does not add continuous background polling after exit.

**Constraints**: Preserve existing Lua configuration shape and aliases; no hidden global observation, daemon, autostart, IPC, persistence, ambient Lua access, or broad device discovery without explicit current-run consent. Hardware-only Fn/layer behavior that does not emit input events is reported unavailable rather than guessed. Trigger support and output support are distinct diagnostics.

**Scale/Scope**: One terminal-started runner or doctor command, current-run configured evdev devices, standard Linux keyboard/media/navigation/keypad key codes, existing macro output backends, and Keychron K5 Pro keys that emit normal key events.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Canonical key identity, aliases, parsing, duplicate detection, and support-status behavior live in `signal-auras-core` before Lua/CLI/adapter wiring.
- Wayland/Compositor Awareness: PASS. KDE Wayland remains the target; evdev/uinput and provider-specific unavailable behavior are explicit and fail closed.
- Rust Safety Boundaries: PASS. Raw evdev reads and uinput writes remain inside `signal-auras-wayland`; the new core key table is pure data and parsing logic.
- Lua Extension Contract: PASS. Existing Lua fields and aliases remain compatible; no new ambient Lua capability is introduced.
- NixOS Reproducibility: PASS. The generated key table is committed, and verification uses flake commands. Any regeneration procedure is documented and does not run at runtime.
- Security and Consent: PASS. Key discovery and physical observation reuse explicit current-run input consent and do not persist observed state.
- TDD and Testability: PASS. Tasks will add failing tests for core key parsing, Lua compatibility, evdev decoding, output mapping, diagnostics, doctor reports, and no persistence before implementation.
- Minimal Composition: PASS. No new service, async runtime, state store, IPC, plugin system, or global registry is introduced.
- No Hidden Global Behavior: PASS. Discovery is an explicit command, runtime observation remains script-configured/current-run, and discovered keys are not cached.
- Incremental Delivery: PASS. P1 trigger parsing, P2 macro output, P3 doctor discovery, and P4 alias compatibility can be delivered and tested independently.

## Project Structure

### Documentation (this feature)

```text
specs/014-full-key-coverage/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── key-vocabulary.md
│   └── key-discovery.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── key.rs
├── hotkey.rs
├── motion.rs
├── macro_plan.rs
├── config.rs
└── lib.rs

crates/signal-auras-lua/src/
└── sandbox.rs

crates/signal-auras-wayland/src/
├── evdev.rs
├── uinput.rs
└── diagnostics.rs

crates/signal-auras-cli/src/
├── main.rs
└── runner.rs

tests/contract/
├── lua_api.rs
├── rust_library.rs
└── cli_runner.rs

tests/integration/
└── runner_flow.rs

tests/compositor/
└── manual-wayland-verification.md

README.md
```

**Structure Decision**: Keep key identity, canonicalization, aliases, and support classification in `signal-auras-core::key` so all Lua surfaces and adapters share one contract. Keep Lua parsing as a consumer of the core parser. Keep raw evdev code decoding and uinput emission in `signal-auras-wayland` behind the existing unsafe input/output boundary. Keep doctor command changes in the CLI runner with simulated tests and supplemental KDE manual verification.

## Complexity Tracking

No constitution violations.
