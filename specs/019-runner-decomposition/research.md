# Research: Runner Architecture Decomposition

## Decision: Gate Structural Refactor Behind Behavior Specs

**Rationale**: Lifecycle cleanup, callback responsiveness, and focus policy unification are correctness contracts. Decomposing `runner.rs` before those contracts have tests would make regressions harder to detect.

**Alternatives considered**: A broad runner rewrite was rejected because it would combine behavior changes with structural movement.

## Decision: Start With Lifecycle Session Ownership

**Rationale**: Current-run resources need one owned cleanup boundary that can be called from startup failure and normal shutdown. A named lifecycle configuration/session type also removes long argument lists without changing public behavior.

**Alternatives considered**: Keeping cleanup as scattered helper calls was rejected because partial startup failures remain difficult to audit.

## Decision: Keep Runtime Loop Coordination Separate From Controller Execution

**Rationale**: Wake ordering, budgets, focus state, and shutdown state are different review concerns from Lua callback execution. Separate boundaries let tests exercise each contract directly.

**Alternatives considered**: A single runner facade was rejected because it would hide the same responsibilities behind a new large type.
