# Data Model: Lua Editor Tooling for Signal Auras Scripts

## Lua Workspace Configuration

**Represents**: Repository-level settings consumed by Lua editor tooling.

**Fields**:

- `runtime.version`: Lua runtime dialect expected by scripts.
- `diagnostics.globals`: Host-provided globals that should not be reported as undefined.
- `workspace.library`: Additional editor metadata paths.
- `workspace.checkThirdParty`: Third-party prompt behavior for LuaLS workspaces.

**Validation Rules**:

- Must target Lua 5.4-compatible scripts.
- Must include all current Signal Auras DSL globals: `macro`, `key`, `text`, and `delay`.
- Must not disable all diagnostics.

## Signal Auras DSL Stub

**Represents**: Editor-only metadata for host-provided Lua DSL functions.

**Fields**:

- `macro(actions)`: Builds an ordered macro definition from action values.
- `key(name)`: Describes a key press action.
- `text(value)`: Describes a text input action.
- `delay(ms)`: Describes a delay action in milliseconds.

**Validation Rules**:

- Must be marked as metadata, not runtime implementation.
- Must not be loaded by the Signal Auras runtime.
- Must document argument shapes for all current globals.

## Editor Verification Procedure

**Represents**: Reproducible or manual steps used to verify editor diagnostics.

**Fields**:

- `tool_availability_command`: Confirms LuaLS is available in the dev shell.
- `workspace_root`: Repository root expected by LuaLS.
- `example_scripts`: Existing scripts used for verification.
- `fallback_manual_steps`: Neovim/LuaLS workflow when command-line diagnostic checking is unavailable.

**Validation Rules**:

- Must reference existing files.
- Must preserve existing Rust runtime verification.
- Must call out that the feature changes editor metadata only.
