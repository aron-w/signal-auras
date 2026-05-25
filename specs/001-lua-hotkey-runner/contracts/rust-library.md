# Rust Library Contract: Lua Hotkey Runner

## Core Crate Responsibilities

`signal-auras-core` owns pure automation semantics:

- validate normalized automation configuration
- normalize and validate hotkey identifiers
- represent scope selections
- decide whether a trigger is allowed for an active process
- plan macro execution in declared order
- enforce v1 repeated-trigger policy
- collect runtime stats
- model diagnosable errors

Core code must not call Wayland, Lua, terminal, filesystem, or OS process APIs except through explicit data supplied by integration layers.

## Adapter Traits

Wayland and CLI integration layers call core logic through explicit contracts.

```rust
pub trait ActiveProcessProvider {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError>;
}

pub trait HotkeyRegistrar {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError>;
    fn unregister_all(&mut self) -> Result<(), DiagnosableError>;
}

pub trait MacroExecutor {
    fn execute_action(&mut self, action: &MacroAction) -> Result<(), DiagnosableError>;
}
```

Exact Rust signatures may change during implementation, but these responsibilities and error boundaries must remain.

## Required Test Contracts

- Config validation rejects malformed script output before registration.
- Scope matcher allows matching process names and denies unknown or non-matching active process.
- Explicit global scope is only constructible through consent-flow data, not absent scope.
- Macro planner preserves action order.
- Repeated trigger for the same running macro is denied in v1.
- Stats counters distinguish triggers, successes, failures, denials, and permission failures.
- Wayland adapter mocks can force unsupported protocol, missing permission, and unavailable active-process errors.
