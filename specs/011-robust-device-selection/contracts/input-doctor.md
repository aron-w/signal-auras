# Contract: Input Doctor Diagnostics

## Command

```text
signal-auras doctor input <lua-file>
```

## Required Output Semantics

- Report whether the Lua file configures an unsafe evdev input provider.
- For selected devices, report each configured path, symlink target if known,
  duplicate status when applicable, access status, and stable
  `/dev/input/by-signal-auras/...` recommendation when applicable.
- For `devices = "all"`, report that broad discovery is current-run only and
  recommend selected stable paths for daily use.
- Report `/dev/uinput` read/write status when uinput output is configured.
- Identify Signal Auras' own virtual output device as excluded when detected by
  the probe.
- Include least-privilege remediation through
  `programs.signal-auras.unsafeInput` without changing permissions.

## Failure Behavior

- The command exits successfully only when required configured permissions are
  available and no blocking selection issue is detected.
- The command MUST NOT start input observation, create a uinput device, persist
  discovered devices, or broaden consent.
