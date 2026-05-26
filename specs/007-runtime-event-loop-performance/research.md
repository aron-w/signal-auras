# Research: Runtime Event Loop Performance

## Decision: Add `mio` as the readiness foundation

`mio` provides a small cross-platform polling abstraction and supports Unix fd
registration without introducing an async runtime. This matches the project
constraint against daemons or broad runtime frameworks.

## Decision: Use incremental macro runs before output-worker expansion

The highest-risk latency bug is blocked input observation during macro delays.
Representing macro execution as a pollable state machine fixes that first while
leaving room for a dedicated output worker if high-throughput output later
requires it.

## Decision: Use `tracing` for diagnostics

`tracing` gives structured level-gated diagnostics with low disabled overhead
and lets verbose logs move to stderr while stdout stays usable for summaries.
