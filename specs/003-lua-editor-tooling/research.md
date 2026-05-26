# Research: Lua Editor Tooling for Signal Auras Scripts

## Decision: Use Repository-Level LuaLS Configuration

**Decision**: Add `.luarc.json` at the repository root.

**Rationale**: Neovim LuaLS setups commonly discover `.luarc.json` from the workspace root. This keeps diagnostics scoped to the repository and avoids editor-specific Neovim configuration.

**Alternatives considered**: Editor-local Neovim settings were rejected because they are not reproducible from the repository. Disabling diagnostics was rejected because real Lua issues must remain visible.

## Decision: Describe the Signal Auras DSL with an EmmyLua/LuaLS Stub

**Decision**: Add `lua-types/signal-auras.lua` with `---@meta` annotations for `macro`, `key`, `text`, and `delay`.

**Rationale**: The existing example scripts are valid inside the Signal Auras host environment because the Rust-backed Lua sandbox provides these globals. LuaLS needs static metadata to know those names and their rough call shapes.

**Alternatives considered**: Listing globals only in diagnostics settings would remove false warnings but would not document call shapes. Generating stubs from Rust was rejected for this small feature because the runtime API is currently tiny and stable.

## Decision: Add LuaLS to the Nix Dev Shell

**Decision**: Add `lua-language-server` to `flake.nix`.

**Rationale**: The project constitution requires reproducible NixOS development and verification paths. Contributors should not need a host-installed language server.

**Alternatives considered**: Documenting external installation was rejected because it creates machine-specific behavior.

## Decision: Keep Formatting Out of Scope

**Decision**: Do not add `stylua` or `stylua.toml` in this feature.

**Rationale**: The reported problem is diagnostics and syntax/editor API understanding. Adding formatting policy would expand scope and create style decisions unrelated to false LuaLS diagnostics.

**Alternatives considered**: Adding `stylua` now was considered but deferred until formatting policy is explicitly requested.
