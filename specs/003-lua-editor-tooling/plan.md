# Implementation Plan: Lua Editor Tooling for Signal Auras Scripts

**Branch**: `003-lua-editor-tooling` | **Date**: 2026-05-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-lua-editor-tooling/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add repository-local Lua editor tooling so Signal Auras scripts can be edited with LuaLS without false diagnostics for the stable DSL globals. The implementation adds a LuaLS workspace configuration, an editor-only EmmyLua/LuaLS metadata stub for `macro`, `key`, `text`, and `delay`, Nix dev-shell availability for LuaLS, and a documented verification path. Existing examples are normalized to valid Lua call syntax, while the runtime parser keeps the legacy `delay 50` spelling compatible.

## Technical Context

**Language/Version**: Existing Rust stable workspace from the project flake; Signal Auras scripts use Lua 5.4-compatible semantics for editor configuration.

**Primary Dependencies**: Existing workspace crates plus the Nix dev-shell package `lua-language-server`. Optional formatting is not introduced in this feature to keep scope limited to diagnostics and API metadata.

**Storage**: Repository files only: `.luarc.json`, `lua-types/signal-auras.lua`, and feature documentation. No persistent runtime storage.

**Testing**: Verify with `nix develop -c lua-language-server --version` when available through the flake, inspect LuaLS workspace configuration, run existing Rust checks with `nix develop -c cargo test`, and use the documented manual Neovim/LuaLS verification for the example scripts if the LuaLS CLI check mode is not supported by the packaged version.

**Target Platform**: NixOS development environment and Neovim workspaces opened at the repository root. Runtime target remains unchanged.

**Project Type**: Existing Rust workspace with Lua scripts as user-facing configuration examples; this feature adds editor/tooling metadata only.

**Performance Goals**: Editor diagnostics for the two existing example scripts should settle without false DSL-global diagnostics during normal LuaLS analysis. No runtime performance impact.

**Constraints**: No runtime Lua sandbox semantic changes, no new script capabilities, no hotkey/compositor/input behavior, no background services, and no hidden global behavior. Tooling dependencies must be represented in the Nix flake. Any parser touch is limited to preserving the existing delay action while accepting valid Lua call syntax.

**Scale/Scope**: Covers the current public Signal Auras Lua DSL globals `macro`, `key`, `text`, and `delay`, plus the two existing examples. Future DSL expansion is out of scope except for documenting how to update the stub.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. This feature does not implement runtime automation behavior; it documents the existing Lua extension surface for editor tooling only.
- Wayland/Compositor Awareness: PASS. No Wayland, compositor, global input, active process, or synthesized-input behavior is introduced.
- Rust Safety Boundaries: PASS. No unsafe Rust, FFI, privileged helper, or protocol boundary changes are introduced.
- Lua Extension Contract: PASS. The feature creates an editor-facing contract for the existing DSL while preserving the Rust-backed Lua sandbox as the runtime authority.
- NixOS Reproducibility: PASS. LuaLS is added to the project flake dev shell and verification commands are documented.
- Security and Consent: PASS. No new sensitive automation capability or permission flow is introduced.
- TDD and Testability: PASS. This is tooling metadata, so verification is configuration inspection plus reproducible tool availability and existing runtime tests; manual editor verification is documented for Neovim/LuaLS UI diagnostics.
- Minimal Composition: PASS. The solution is static workspace metadata and one dev-shell package, with no services or new abstractions.
- No Hidden Global Behavior: PASS. The Lua globals are editor declarations only and do not install hooks, hotkeys, persistence, IPC, or background processes.
- Incremental Delivery: PASS. The feature is an independently usable editor-tooling increment.

## Project Structure

### Documentation (this feature)

```text
specs/003-lua-editor-tooling/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── lua-editor-api.md
└── tasks.md
```

### Source Code (repository root)

```text
.luarc.json                 # LuaLS workspace configuration
lua-types/
└── signal-auras.lua        # Editor-only DSL metadata
flake.nix                   # Reproducible dev-shell LuaLS package
examples/
├── poe2-legacy.lua
└── prompt-scope.lua
```

**Structure Decision**: Keep runtime crates untouched. Add editor metadata at the repository root where LuaLS and Neovim discover workspace configuration, and keep the API stub under `lua-types/` to make the editor-only contract explicit.

## Complexity Tracking

No constitution violations.
