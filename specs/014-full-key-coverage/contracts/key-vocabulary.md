# Contract: Key Vocabulary and Lua Compatibility

## Purpose

All user-facing key names resolve through one core vocabulary before triggers,
macro actions, duplicate detection, diagnostics, or backend planning use them.

## Accepted Surfaces

The same parser accepts key names in:

- `leader`
- `motions[].trigger`
- `motions[].repeat.while_held`
- `bindings[].trigger.key`
- legacy `hotkeys` table keys
- `macro { key "..." }`

Mouse tokens such as `<LClick>` and wheel tokens remain separate motion tokens.

## Parsing Rules

- Trim surrounding whitespace from key names.
- Match canonical names and aliases case-insensitively unless a future plan records a compatibility reason for case-sensitive behavior.
- Normalize one-character printable keys to their canonical key token.
- Normalize legacy aliases to canonical names before storage.
- Reject empty names and names that do not map to a known canonical key token.

## Compatibility Aliases

The following existing spellings must remain accepted:

- one-character printable keys currently accepted by hotkeys or macros
- `F1` through `F24`
- `Left`, `Right`
- `Enter`, `Return`
- `Tab`
- `Esc`, `Escape`
- `Delete`, `Del`
- `Backspace`
- `Space`

Additional generated Linux-style names may be accepted as aliases when they
resolve to a standard key.

## Duplicate Detection

Duplicate binding and motion validation compares normalized canonical key
tokens. Two scripts that spell the same key differently must be treated as the
same trigger.

## Output Planning

Macro `key` actions store or resolve the canonical key identity. The output
backend then reports support independently:

- supported: emit the requested key
- unsupported: report unsupported output and do not substitute another key
- denied/unavailable: fail closed with the existing permission/backend error

## Diagnostics

User-visible diagnostics show:

- canonical token name first
- aliases where useful for migration or discovery
- raw evdev code when a physical event is involved
- separate triggerability and emittability status
