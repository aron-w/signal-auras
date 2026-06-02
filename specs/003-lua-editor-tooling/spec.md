# Feature Specification: Lua Editor Tooling for Signal Auras Scripts

**Feature Branch**: `003-lua-editor-tooling`

**Created**: 2026-05-26

**Status**: Draft

**Input**: User description: "Signal-Auras-Lua-Scripts sollen in Neovim mit lua-language-server ohne falsche Diagnostics bearbeitbar sein. Die projektspezifische Lua-DSL mit macro, key, text und delay soll als Editor-API beschrieben werden. Die Toolchain soll ueber Nix reproduzierbar sein."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit Signal Auras Scripts Without False DSL Diagnostics (Priority: P1)

A developer opens an existing Signal Auras Lua script in Neovim and expects editor diagnostics to distinguish real Lua mistakes from valid Signal Auras DSL globals.

**Why this priority**: This is the smallest valuable increment. It removes false `undefined-global` diagnostics for the current public DSL without changing runtime behavior.

**Independent Test**: Open or check the existing example scripts with the configured Lua language tooling and confirm `macro`, `key`, `text`, and `delay` are accepted as valid Signal Auras globals while normal Lua diagnostics remain enabled.

**Acceptance Scenarios**:

1. **Given** the repository is opened as a workspace, **When** `examples/poe2-legacy.lua` is checked by Lua editor tooling, **Then** the Signal Auras DSL globals are recognized and do not produce false undefined-global diagnostics.
2. **Given** the repository is opened as a workspace, **When** `examples/prompt-scope.lua` is checked by Lua editor tooling, **Then** the Signal Auras DSL globals are recognized and do not produce false undefined-global diagnostics.
3. **Given** a Lua script contains an unrelated syntax or type issue, **When** Lua editor tooling runs, **Then** the issue remains visible instead of diagnostics being globally disabled.

---

### User Story 2 - Discover the Lua DSL Shape in the Editor (Priority: P2)

A developer writing or reviewing Signal Auras scripts expects the editor to expose the intended shape of the DSL calls used by supported scripts.

**Why this priority**: Recognizing globals removes noise, but a documented editor API makes the Lua extension layer easier to use and review.

**Independent Test**: Inspect the editor API stub and confirm it documents the supported DSL globals and their expected argument shapes without adding runtime code.

**Acceptance Scenarios**:

1. **Given** a developer views the editor API description, **When** they inspect `macro`, `key`, `text`, and `delay`, **Then** the expected argument and return shapes are documented.
2. **Given** the editor API description exists, **When** Signal Auras runs scripts, **Then** runtime semantics remain owned by the existing Rust-backed Lua sandbox.

---

### User Story 3 - Reproduce Lua Editor Tooling Through Nix (Priority: P3)

A contributor enters the development shell and expects the Lua editor tooling used for verification to be available without out-of-band installation.

**Why this priority**: Nix reproducibility is a project constraint, and editor diagnostics should not depend on untracked local packages.

**Independent Test**: Enter the project development shell and confirm the Lua language server is available for editor or command-line verification.

**Acceptance Scenarios**:

1. **Given** a contributor enters the development shell, **When** they ask for the Lua language server version or run the documented check, **Then** the required tool is available.
2. **Given** the documented verification command is run in the development shell, **When** the example scripts are checked, **Then** the result is reproducible from repository state.

### Edge Cases

- If a user opens a single Lua file outside the repository root, workspace-level configuration may not be discovered; documentation must state that verification assumes the repository root is the workspace.
- If future DSL globals are added by the runtime, editor metadata can become stale; the DSL stub must be kept as a versioned editor-facing contract.
- If Lua language server command-line check behavior differs by version, the quickstart must provide a fallback manual Neovim verification path.
- If the language server reports diagnostics unrelated to the Signal Auras DSL, those diagnostics should remain visible and should not be suppressed by this feature.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide workspace-level Lua editor configuration for Signal Auras scripts.
- **FR-002**: The editor configuration MUST target Lua 5.4-compatible script semantics.
- **FR-003**: The editor configuration MUST recognize the Signal Auras DSL globals `macro`, `key`, `text`, and `delay` without disabling unrelated Lua diagnostics.
- **FR-004**: The system MUST provide an editor API description for `macro`, `key`, `text`, and `delay`.
- **FR-005**: The editor API description MUST be documentation/type metadata only and MUST NOT change runtime Lua sandbox behavior.
- **FR-006**: The Nix development shell MUST include the Lua language server needed for reproducible editor diagnostics.
- **FR-007**: Documentation MUST include a reproducible verification path for the existing example scripts and a manual editor fallback if command-line diagnostics are unavailable.
- **FR-008**: The feature MUST NOT introduce hidden global hotkeys, hooks, background processes, persistent state, network access, filesystem access, compositor access, or new script capabilities.
- **FR-009**: The feature MUST preserve the Rust core and Lua sandbox as the source of runtime automation semantics.
- **FR-010**: The editor-facing DSL contract MUST be easy to update when the public Signal Auras Lua API evolves.

### Key Entities *(include if feature involves data)*

- **Lua Workspace Configuration**: Repository-level editor settings that describe Lua version, diagnostics behavior, and local editor API metadata.
- **Signal Auras DSL Stub**: Editor-only metadata for the supported Lua DSL globals and their expected argument shapes.
- **Editor Verification Procedure**: Documented steps for confirming the examples no longer produce false DSL diagnostics.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Both existing example scripts can be checked from the repository workspace without undefined-global diagnostics for `macro`, `key`, `text`, or `delay`.
- **SC-002**: The editor API description lists all four accepted DSL globals and documents their arguments in one discoverable file.
- **SC-003**: A contributor can enter the Nix development shell and find the Lua language server without installing it separately.
- **SC-004**: Runtime Rust tests continue to pass, demonstrating that editor tooling did not alter script execution semantics.
- **SC-005**: The feature verification path is documented with exact commands or manual editor steps.
- **SC-006**: No new sensitive automation capability, permission, background service, compositor integration, or script runtime access is introduced.

## Assumptions

- Neovim uses `lua-language-server`/LuaLS for Lua diagnostics.
- The repository root is the expected workspace root for editor configuration discovery.
- `macro`, `key`, `text`, and `delay` are the current editor-facing DSL globals to document.
- The feature is limited to editor diagnostics and documentation metadata; formatting policy is optional and runtime behavior is out of scope.
