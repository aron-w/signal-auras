# Unsafe Evdev Provider Contract

The evdev provider is an explicit high-trust local backend for systems where a
safe compositor motion provider is unavailable. It is not enabled by default and
never scans `/dev/input` automatically.

## Configuration

Scripts opt in by declaring:

```lua
input_provider = {
  backend = "evdev",
  mode = "grab",
  output = "uinput",
  devices = "all",
  acknowledge_risk = "GRAB_ALL_INPUTS",
}
```

- `backend = "evdev"` reads Linux evdev input events.
- `mode = "observe"` reports events but does not suppress them.
- `mode = "grab"` requests evdev exclusive delivery for the configured devices.
- `output = "portal"` keeps generated input routed through the KDE
  RemoteDesktop portal.
- `output = "uinput"` emits generated input through `/dev/uinput`.
- `devices` must contain one or more explicit device paths, or the literal
  string `"all"` to scan `/dev/input/event*`.
- `acknowledge_risk = "GRAB_ALL_INPUTS"` is required when `mode = "grab"` and
  `devices = "all"` are combined.

## Capability Behavior

When the configured device files open successfully, the provider may satisfy
`CompositePointerObservation`. It satisfies `CompositePointerConsumption` only
when `mode = "grab"` succeeds. Observe mode must not claim consumption, because
it cannot prevent the original event from reaching the focused application.

When `/dev/uinput` opens and the virtual keyboard/pointer device is created
successfully, `output = "uinput"` may satisfy `SynthesizedInput`.

Device-open failures are registration failures with a diagnostic that names the
evdev source and recommends granting read access only to the explicit device
files.

## Event Semantics

The provider emits normalized `MotionInputEvent` values into the same runtime
used by scripted tests. The runner remains responsible for:

- matching motion sequences
- checking scope
- executing macros
- scheduling repeat ticks
- cancelling repeats when any `while_held` token is released

The current implementation maps configured function-key leaders, the `f` key,
and left/right/middle mouse buttons for observation. Uinput output supports the
named keys used by existing macros, function keys, ASCII letters/digits, space,
slash, and left/right/middle mouse clicks. Additional key mapping should be
added explicitly instead of passing through raw key streams.
