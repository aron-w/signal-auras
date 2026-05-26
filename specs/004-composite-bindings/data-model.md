# Data Model: Composite Input Bindings

## ModifierSet

Canonical ordered set of `Ctrl`, `Alt`, `Shift`, and `Super`. Duplicate or unknown values are invalid.

## MouseTrigger

Primary pointer input:

- `button`: `left`, `right`, or `middle`
- `wheel`: `up` or `down`

## BindingTrigger

One primary trigger:

- `Keyboard(HotkeyId)`
- `Composite(ModifierSet, MouseTrigger)`

## BindingMode

- `consume`: suppress the original trigger event when supported. This is the default.
- `passthrough`: allow the original trigger event and still execute the macro.

## BindingDefinition

Lua-facing normalized binding: trigger, mode, and macro definition.

## Runtime Binding

Runtime binding: trigger, scope selection, mode, macro definition, and registration state.
