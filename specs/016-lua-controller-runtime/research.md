# Research: Lua Controller Runtime

## Decision: Two-phase controller model

Rust first collects and validates controller registrations, capabilities, scopes, trigger normalization, and overload policy before any provider activation.

**Rationale**: This satisfies no-hidden-global-behavior and keeps registration side-effect free. It also lets CLI and Wayland integration fail before input capture when scripts are invalid.

**Alternatives considered**: Dynamic registration during callbacks was rejected for V1 because it complicates capability revocation, duplicate detection, and queue bounds.

## Decision: Library contracts before embedded Lua execution

This increment adds pure Rust contracts for `ControllerRegistrationSet`, `LuaCallbackScheduler`, and `RustOperationBatch`, then exposes a parser-backed controller loader.

**Rationale**: The constitution requires library-first behavior. The current project does not embed a Lua VM yet, so contracts can be tested now without adding a runtime dependency prematurely.

**Alternatives considered**: Adding a Lua VM immediately was deferred because it would widen dependency and sandbox scope before the scheduling/output contracts are stable.

## Decision: Rooted local module loader

Controller imports use `sa.import("module")`, resolved under the main script directory. Absolute paths, parent traversal, `require`, `package`, `dofile`, and `loadfile` are denied.

**Rationale**: Multi-file controllers are required, but ambient filesystem/package access is not. Rooted imports give deterministic startup behavior and auditable script scope.

**Alternatives considered**: Standard Lua `require` was rejected because it depends on ambient package paths and can escape the script root.

## Decision: Per-trigger bounded callback scheduling

Each controller trigger can have at most one active or pending callback task by default. Repeated triggers are accepted, skipped, denied, or dropped with explicit disposition.

**Rationale**: This follows the callback wakeup and repeat overload plans: input polling and shutdown must remain serviceable while user Lua work is pending.

**Alternatives considered**: Unbounded callback queues were rejected because they can convert rapid input into unbounded latency and memory growth.

## Decision: Rust-backed output batching

Lua output APIs enqueue Rust operation requests, currently modeled as ordered synthesized input requests guarded by `SynthesizedInput` capability.

**Rationale**: Lua expresses policy; Rust owns OS-facing emission, batching, permission denial, and diagnostics.

**Alternatives considered**: Direct output calls from Lua were rejected because they would block or cross sensitive Wayland/uinput boundaries from script code.
