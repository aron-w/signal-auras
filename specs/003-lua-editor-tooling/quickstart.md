# Quickstart: Lua Editor Tooling Verification

## Reproducible Tool Availability

From the repository root:

```sh
nix develop -c lua-language-server --version
```

This confirms the language server is available from the flake development shell.

## Manual Neovim/LuaLS Verification

1. Open Neovim at the repository root.
2. Open `examples/poe2-hideout.lua`.
3. Confirm LuaLS does not report `macro`, `key`, `text`, or `delay` as undefined globals.
4. Open `examples/prompt-scope.lua`.
5. Confirm the same DSL globals do not produce false undefined-global diagnostics.
6. Introduce a temporary unrelated Lua syntax error and confirm Lua diagnostics still appear, then revert the temporary edit.

## Runtime Regression Verification

```sh
nix develop -c cargo test
```

The editor metadata must not change runtime Lua sandbox behavior.
