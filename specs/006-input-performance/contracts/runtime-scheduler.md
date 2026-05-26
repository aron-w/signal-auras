# Runtime Scheduler Contract

## Scope

The live CLI runner must coordinate shortcut input, evdev motion input, repeat deadlines, hotplug rescans, macro execution, and Ctrl-C shutdown without fixed-delay input polling.

## Required Behavior

- Compute the next wait deadline from active repeat timers, hotplug rescan timers, and shutdown checks.
- Wait for evdev readiness until the next deadline when an evdev provider is configured.
- Process all ready input batches before executing due repeat ticks.
- Cancel repeats before executing any repeat tick after a release event has been processed.
- Emit verbose diagnostics for motion input, dispatch latency, repeat lifecycle, and provider rescans.

## Non-Goals

- No new Lua DSL fields or motion tokens.
- No daemon, background service, persistent state, or IPC endpoint.
