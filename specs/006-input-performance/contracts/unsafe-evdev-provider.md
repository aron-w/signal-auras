# Unsafe Evdev Provider Contract

## Scope

The unsafe evdev provider observes `/dev/input/event*` devices during the current runner process when a Lua script explicitly opts into `input_provider.backend = "evdev"`.

## Required Behavior

- Explicit device lists open only the configured paths.
- `devices = "all"` discovers current `/dev/input/event*` paths and rescans during the run.
- Removed devices are marked inactive and logged.
- Newly readable devices discovered during `devices = "all"` are opened and included in fair polling.
- Readiness waits block until an active fd is readable or the caller-provided deadline expires.
- Device event dispatch is fair across active devices.
- Grab mode and uinput output preserve the existing explicit permission and fail-closed behavior.

## Diagnostics

Verbose/runtime diagnostics must include active device count, removed paths, added paths, skipped unreadable paths, and source device for observed input events.
