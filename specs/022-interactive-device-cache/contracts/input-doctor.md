# Contract: Input Doctor Interactive Cache Diagnostics

## Command

```text
signal-auras doctor input <lua-file>
```

## Required Output Semantics

- Report whether the Lua file uses interactive evdev device selection.
- Report the derived runtime cache path when interactive selection is configured.
- Report cache status: missing, accepted, stale, invalid, permission-incomplete,
  or unsafe-runtime-dir.
- Report selected evdev path status, current device identity status, stable path
  recommendation, and `/dev/uinput` status when relevant.
- Include remediation for interactive startup and durable NixOS selected-device
  permissions.

## Side-Effect Rules

- The command MUST NOT prompt for selection.
- The command MUST NOT grant permissions.
- The command MUST NOT start input observation, grab devices, create uinput
  output, or rewrite the cache.
