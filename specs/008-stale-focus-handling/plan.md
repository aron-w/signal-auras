# Implementation Plan: Stale Focus Handling

**Branch**: `008-stale-focus-handling` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-stale-focus-handling/spec.md`

## Summary

Make process-aware macro matching fail closed when focused-process metadata is stale, missing, denied, or otherwise untrusted. The implementation keeps the Lua configuration API unchanged, adds a small Rust freshness and denial model around existing active-process contexts, preserves the original KDE/KWin callback receipt timestamp in the live bridge cache, uses the existing KDE/Wayland provider boundary, refreshes the live KDE active-process snapshot through a 1 second KWin heartbeat callback, and improves diagnostics so stale denials identify the configured rule, age, threshold, and reason without logging private command-line or window text.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates, `mio`/`nix`/`udev`/`tracing` dependencies already introduced by the runtime event-loop performance work. No new dependency is required.

**Storage**: Repository files only. No persistent focus cache, daemon state, IPC state, or background metadata store.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated tests cover freshness boundary behavior, unavailable/denied metadata, recovery after fresh metadata arrives, no macro emission on denial, live KDE cached-focus timestamp preservation, cached matching metadata becoming stale without a new callback, generated KWin heartbeat script contents, and diagnostic classification.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing explicit current-run consent for process metadata and synthesized input.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Scope evaluation remains an in-memory constant-time check for each trigger. No new blocking compositor query is added to the hot path.

**Constraints**: Preserve Lua process-scope syntax, default stale threshold of 2 seconds, explicit current-run process inspection consent, fail-closed behavior for unavailable compositor metadata, privacy-bounded diagnostics, and no hidden global behavior. Reading cached focus state must be side-effect free for freshness; only KWin focus/heartbeat callbacks may create fresh KDE focus snapshots.

**Scale/Scope**: One terminal-started runner process, one current focus snapshot at a time from the existing KDE active-window bridge, process-scoped hotkeys and motions, and existing verbose diagnostics.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Freshness policy and denial classification live in `signal-auras-core` before CLI/adapter formatting.
- Wayland/Compositor Awareness: PASS. KDE Wayland remains the metadata target; unsupported, unavailable, denied, delayed, or stopped heartbeat metadata fails closed.
- Rust Safety Boundaries: PASS. Process metadata stays behind the existing Wayland adapter/provider boundary; no unsafe or privileged scope is expanded.
- Lua Extension Contract: PASS. Existing `scope = { processes = { ... } }` remains unchanged.
- NixOS Reproducibility: PASS. Verification uses the project flake commands and adds no new dependency.
- Security and Consent: PASS. Process inspection remains explicit, current-run scoped, and revocable through existing capability checks.
- TDD and Testability: PASS. Core freshness/denial tests and runner no-emission tests are planned before implementation.
- Minimal Composition: PASS. No daemon, IPC, async runtime, registry, or persistent cache is introduced; the heartbeat is scoped to the already installed current-run KWin monitor script.
- No Hidden Global Behavior: PASS. Defaults remain inert except for user-configured current-run bindings and the current-run active-process monitor required by process-scoped matching.
- Incremental Delivery: PASS. Stale denial, metadata recovery, and diagnostics are separable user-story increments over existing process-aware matching.

## Project Structure

### Documentation (this feature)

```text
specs/008-stale-focus-handling/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── focus-freshness.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── scope.rs
├── lib.rs
└── stats.rs

crates/signal-auras-wayland/src/
├── process.rs
├── adapter.rs
└── kde.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/contract/
├── rust_library.rs
└── cli_runner.rs

tests/integration/
└── runner_flow.rs
```

**Structure Decision**: Keep freshness and privacy-preserving denial classification in `signal-auras-core::scope`, keep provider timestamp creation inside existing Wayland adapter contexts, preserve live KDE bridge callback timestamps in `signal-auras-wayland::kde_bridge`, add the KWin heartbeat inside the existing active-process monitor script, and keep CLI changes limited to logging/metrics and no-emission behavior.

## Complexity Tracking

No constitution violations.
