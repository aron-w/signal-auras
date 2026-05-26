# Research: Composite Input Bindings

## Decision: Use a Unified Binding Model

`hotkeys` remain a Lua compatibility surface, but the core stores all configured automations as normalized bindings. This keeps duplicate detection, scope handling, macro scheduling, stats, and capability requirements in one path.

## Decision: Fail Closed for Consumed Pointer Events

Consumed mouse bindings require both pointer observation and pointer consumption capabilities. The current KDE adapter reports these capabilities as unsupported, so activation fails before registration. This is preferable to allowing accidental scroll or click side effects.

## Decision: Keep Output Semantics in Macro Execution

Macro actions continue to be emitted from the existing macro plan. Trigger modifiers are represented only in the trigger model and do not alter macro actions such as `key "Alt+Right"`.

## Alternatives Considered

- Reusing string hotkey identifiers for pointer triggers was rejected because validation and future keyboard trigger support need structured fields.
- Registering passthrough pointer bindings through the keyboard shortcut provider was rejected because pointer observation is a separate capability.
