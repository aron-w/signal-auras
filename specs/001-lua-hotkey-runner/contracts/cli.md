# CLI Contract: Lua Hotkey Runner

## Command

```text
signal-auras run <lua-file>
```

## Argument Rules

- The command accepts exactly one Lua file path.
- Zero paths exit before script loading with a diagnosable argument error.
- More than one path exits before script loading with a diagnosable argument error.
- The path must be readable by the host.

## Scope Prompt Contract

When the Lua configuration omits scope, the CLI must prompt in the terminal before registration:

```text
No scope declared by script.
Select scope for this run:
1. Process names
2. Global hotkeys for this run
3. Cancel
```

Process-name selection asks for one or more process names and logs the resulting scope. Global selection requires a separate explicit confirmation before registration. Cancel exits without registration.

## Required Log Events

The runner must emit user-visible terminal output for:

- startup script path
- script validation result
- effective scope
- explicit global selection when used
- capability probe results or diagnosable failure
- hotkey registration result per binding
- denied trigger with active process and configured scope when available
- macro success or failure
- Ctrl-C shutdown start
- final summary stats

## Exit Behavior

- Startup validation failure: non-zero exit; no hotkeys registered.
- Capability or permission failure before registration: non-zero exit; no hotkeys registered.
- Prompt cancel: zero exit; no hotkeys registered.
- Ctrl-C during a successful run: zero exit after unregistering hotkeys and printing stats.
- Runtime unrecoverable error: non-zero exit after unregistering registered hotkeys and printing stats.
