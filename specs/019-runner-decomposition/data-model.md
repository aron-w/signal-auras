# Data Model: Runner Architecture Decomposition

## LifecycleConfig

- Startup inputs required to construct current-run resources.
- Contains only named, validated inputs; it does not own live resources.
- Validation errors are diagnosable before side effects where practical.

## RuntimeSession

- Owns current-run resources such as input sessions, output sessions, callback registrations, bridge scripts, and cleanup state.
- Cleanup is idempotent and produces a report of attempted releases, successes, and failures.
- Startup failure and normal shutdown use the same cleanup boundary where practical.

## RuntimeLoopCoordinator

- Coordinates wake sources: input, callbacks, timers, hotplug, repeats, focus freshness, and shutdown.
- Does not parse CLI arguments or execute Lua directly.
- Preserves no-new-work-after-shutdown semantics.

## ControllerExecutor

- Runs Lua controller callbacks through Rust-owned capability checks, budgets, wakeups, and diagnostics.
- Does not receive OS handles or ambient process/input/screen access.

## DiagnosticsContext

- Carries privacy-bounded diagnostic fields for lifecycle, wakeup, focus, callback, controller, and cleanup decisions.
- Excludes private command-line arguments, window titles, text payloads, and unrelated process metadata.
