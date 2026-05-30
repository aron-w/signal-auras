# Contract: Key Discovery Doctor

## Command

Planned CLI shape:

```text
signal-auras doctor keys <lua-file>
```

The command loads the Lua file, validates the configured input provider, and
requires the same explicit current-run consent path as runtime physical input
observation.

## Non-Goals

- Does not persist discovered keys or aliases
- Does not register hotkeys for later runs
- Does not infer hardware-only Fn/layer/firmware behavior
- Does not synthesize macro output unless an explicit output-support probe is
  later specified and consented
- Does not grant Lua scripts new host capabilities

## Report Fields

Each observed or unavailable key diagnostic includes only:

- current-run device path or device status
- raw key code when an event exists
- canonical token name when known
- aliases when known
- triggerability status
- emittability status for configured output backend
- unavailable reason when unsupported, denied, unknown, or unobserved

## Exit Behavior

- Success when discovery starts and exits normally, even if some keys are
  unobserved or unsupported and reported as such
- Failure before observation when required input consent, devices, or
  permissions are unavailable
- Failure before output support is reported as supported when the output backend
  is denied or unavailable

## Privacy and Safety

Discovery diagnostics do not include macro payloads, text input, process command
lines, window titles, unrelated device data, or persistent identifiers beyond
the current-run device path/status needed for remediation.
