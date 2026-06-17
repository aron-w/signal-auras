# Contract: Runtime Device Cache

## Location

```text
$XDG_RUNTIME_DIR/signal-auras/input-devices/<cache-key>.cache
```

The cache key is derived from the canonical main Lua path.

## Required Semantics

- Cache files are current-user runtime state and may disappear between sessions.
- Accepted cache entries resolve to strict selected evdev paths.
- Cache validation checks script path, provider mode, provider output, selected
  path access, current device identity, stable path target, and own-device
  exclusion.
- Cache writes happen only after successful interactive selection and
  post-repair validation.
- `signal-auras run --reset-input-cache <lua-file>` discards the derived cache
  for that startup before validation and requires a fresh interactive selection.

## Failure Behavior

- Missing, malformed, stale, or permission-incomplete cache entries are not
  partially used.
- Unsafe `$XDG_RUNTIME_DIR` conditions fail closed.
- Deleting the cache revokes automatic reuse for the next startup.
- Explicit cache reset has the same revocation effect for the current startup
  while preserving the normal cache location and validation rules.
