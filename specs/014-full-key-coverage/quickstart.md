# Quickstart: Full Keyboard Key Coverage

## Automated Verification

Run from the repository root:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

If a Nix command is unavailable in the current environment, record the exact
failure and run the corresponding `cargo` command in the existing development
environment when possible.

## Expected Automated Coverage

- Core parser accepts representative standard key categories and legacy aliases.
- Lua validation accepts the shared key vocabulary in `leader`, motions,
  repeat holds, structured binding keys, hotkeys, and macro `key` actions.
- Duplicate detection treats alias-equivalent keys as the same trigger.
- Evdev raw key decoding maps standard key codes to canonical tokens.
- Unknown/vendor/non-key events preserve raw code diagnostics without guessed
  token names.
- Uinput output emits supported canonical keys and reports unsupported backend
  keys without substitution.
- `doctor keys` reports current-run device status, raw code, canonical name,
  aliases, triggerability, emittability, and unavailable reasons without
  persisting state.

## Manual KDE/Hardware Verification

1. Configure an explicit selected keyboard device path in a local Lua file.
2. Run `nix develop -c cargo run -p signal-auras-cli -- doctor keys <file.lua>`.
3. Press representative Keychron K5 Pro letter, number, punctuation, function,
   navigation, keypad, and media keys.
4. Confirm observed standard keys show canonical names and aliases where
   applicable.
5. Press Fn/layer/firmware-only controls that do not emit normal Linux input
   events and confirm the command reports no observable key rather than a
   guessed token.
6. Configure a macro that emits supported navigation/keypad/media keys through
   the selected output backend and confirm supported keys emit while unsupported
   keys fail closed with diagnostics.
7. Stop discovery and rerun it; confirm no previous observed key state is
   remembered.
