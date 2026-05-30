# Research: Full Keyboard Key Coverage

## Decision: Use a Generated Upstream Linux Key Table in Core

Use a generated, committed Rust table derived from Linux `input-event-codes.h` key definitions as the canonical raw-code source. The table is owned by `signal-auras-core` and exposes pure lookup helpers for raw evdev key codes, canonical token names, and aliases.

**Rationale**: The spec requires avoiding a hand-maintained partial mapping. A generated committed table keeps runtime behavior reproducible in Nix builds, avoids broad new dependencies, and makes reviews focus on the generator/source provenance instead of scattered constants.

**Alternatives considered**:
- Add a maintained input library such as an evdev key enum crate: acceptable only if already available and low-scope in the flake, but not required for this feature and may add dependency/update friction.
- Keep extending current local `match` statements: rejected because it repeats the existing partial-list failure mode.
- Generate from the host header at build time: rejected because it makes builds vary by host kernel header availability.

## Decision: Canonical Tokens With Backward-Compatible Aliases

Represent each key as one canonical token name plus zero or more accepted aliases. Lua-facing inputs normalize once before storage and duplicate detection.

**Rationale**: This preserves existing script compatibility while making trigger matching, diagnostics, and output planning consistent across `leader`, motions, structured bindings, legacy hotkeys, and macro `key` actions.

**Alternatives considered**:
- Preserve user spelling internally: rejected because alias-equivalent duplicates would remain hard to detect.
- Break aliases in favor of only generated Linux names: rejected because current scripts using `Esc`, `Return`, `Left`, `Right`, and one-character keys must keep working.

## Decision: Keep Trigger Support and Output Support Separate

The core key identity model records the key identity, while adapters report whether a key is observable as a trigger and emittable by the selected output backend.

**Rationale**: A physical key may be observable from evdev but unavailable through a compositor shortcut provider, and an output backend may not synthesize every known key. Users need precise diagnostics instead of a single supported/unsupported flag.

**Alternatives considered**:
- Treat canonical keys as universally supported: rejected because it would hide provider/backend limitations.
- Maintain separate trigger and output vocabularies: rejected because the spec requires the same user-visible names where backend support exists.

## Decision: Extend Existing Doctor Surface With Explicit Key Discovery

Add an explicit key discovery doctor path, planned as `signal-auras doctor keys <lua-file>`, that uses the script's current-run input provider consent and reports observed key events without persistence.

**Rationale**: Existing `doctor input` checks device access without grabbing or observing live key events. Key discovery is a stronger operation because it observes current-run physical keypresses, so it should be explicit and visibly separate.

**Alternatives considered**:
- Fold live discovery into `doctor input`: rejected because it would make a previously passive diagnostic command observe input unexpectedly.
- Add persistent learned aliases: rejected by the spec and constitution no-persistence rules.

## Decision: Report Hardware-Only Non-Events as Unobserved

Discovery reports that no input event was observed for Fn/layer/firmware-only controls rather than assigning a token.

**Rationale**: Some keyboard behavior never reaches Linux input. Guessing token names for unobserved hardware behavior would create false configuration confidence and hidden assumptions.

**Alternatives considered**:
- Use keyboard-model-specific layout knowledge: rejected because the target universe is standard Linux evdev keys, not vendor firmware internals.
- Ask the user to manually name hidden keys: rejected because runtime cannot trigger on events it never observes.
