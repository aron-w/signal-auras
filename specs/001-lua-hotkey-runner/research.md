# Research: Lua Hotkey Runner

## Decision: Create a Rust workspace with four crates

**Rationale**: The constitution requires a Rust-owned, library-first automation core. A workspace with `signal-auras-core`, `signal-auras-lua`, `signal-auras-wayland`, and `signal-auras-cli` keeps pure behavior independently testable while allowing integration layers to remain thin.

**Alternatives considered**:
- Single binary crate: rejected because core behavior would be harder to test and reuse.
- CLI plus Lua in one crate: rejected because script isolation and host orchestration are separate safety concerns.
- Daemon-first architecture: rejected because v1 is a terminal runner with no persistence, IPC, or autostart.

## Decision: Use explicit adapter traits for Wayland capabilities

**Rationale**: Wayland global shortcuts, active-process metadata, and synthesized input depend on compositor protocols, portals, and permissions. A trait boundary lets core tests use mocks while real adapters detect capabilities and return diagnosable errors.

**Alternatives considered**:
- Hard-code one compositor protocol in core: rejected because it couples pure automation semantics to one desktop.
- Pretend unsupported protocols are no-ops: rejected by the constitution and spec requirement for explicit failures.
- X11 fallback: rejected because X11 compatibility is out of scope for v1.

## Decision: Use a Lua sandbox with a tiny constructor API

**Rationale**: The Lua file should describe configuration, not receive host capabilities. The host will load the file in an environment containing only the approved constructors (`macro`, `key`, `text`, `delay`) and no standard filesystem, network, process, shell, environment, compositor, or global-input APIs.

**Alternatives considered**:
- Full Lua standard library: rejected because it grants ambient capabilities.
- JSON/TOML config instead of Lua: rejected because Lua is the stable extension layer in the project goal.
- Direct Lua callbacks for hotkeys: rejected for v1 because macro planning should be validated before registration and execution.

## Decision: Deny overlapping macro execution for the same hotkey in v1

**Rationale**: Denial is deterministic, auditable, and easy to explain in stats. Queueing introduces scheduling and cancellation semantics that are not needed for the first runner.

**Alternatives considered**:
- Queue repeated triggers: rejected because it can surprise users with delayed input.
- Run overlapping macros concurrently: rejected because it can interleave synthesized input and break macro ordering.

## Decision: Prompt scope in the terminal and keep it current-run only

**Rationale**: The spec locks v1 to terminal prompts and prohibits hidden global behavior. Current-run scope avoids persistent consent state and keeps revocation simple: Ctrl-C ends the run and unregisters hotkeys.

**Alternatives considered**:
- Persist selected scope next to the script: rejected as out of scope and a hidden state risk.
- Default missing scope to global: rejected by explicit consent and no-hidden-global requirements.
- Graphical permission UI: rejected because v1 prompt UX is terminal-only.

## Decision: Use structured terminal logs and final summary counters

**Rationale**: Users need to understand what registered, why a macro was denied, what failed, and what happened during shutdown. Structured logs also make CLI contract tests simpler.

**Alternatives considered**:
- Plain ad hoc print statements: rejected because fields would be harder to test consistently.
- Silent success with only errors: rejected because consent and registration results must be visible.

## Decision: Use automated adapter-contract tests plus manual compositor verification

**Rationale**: Pure behavior and adapter contracts can be tested in Cargo. Real global shortcut, active-window/process metadata, and synthesized-input behavior may require compositor-specific sessions that are not yet available as an automated harness in this empty repository.

**Alternatives considered**:
- Skip compositor verification: rejected because the feature depends on Wayland behavior.
- Block all planning until a full compositor harness exists: rejected because v1 can still define exact manual verification while library behavior remains automated.

## Decision: Update the Nix flake for Rust and native tooling during implementation

**Rationale**: The current flake only provides Spec Kit development tools. The implementation needs Cargo/Rust tooling and any native libraries required by Lua or Wayland crates to satisfy NixOS-first reproducibility.

**Alternatives considered**:
- Rely on host-installed Rust: rejected because verification must be reproducible through Nix.
- Vendor binaries outside Nix: rejected because native dependencies must be represented in the flake or explicitly documented.
