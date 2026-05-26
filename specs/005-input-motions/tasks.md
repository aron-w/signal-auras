# Tasks: Unified Input Motions

## Phase 1: Core Model

- [x] Add motion token, trigger, repeat, defaults, and motion definition types.
- [x] Reject empty triggers and duplicate motion triggers.
- [x] Add mouse-click macro action.
- [x] Add inter-action delay execution helper.

## Phase 2: Lua Contract

- [x] Parse `leader`, `defaults`, and `motions`.
- [x] Parse uniform keyboard and mouse trigger lists.
- [x] Validate repeat held state, interval bounds, and emitted macro.
- [x] Validate global and motion-level delay values.
- [x] Preserve `hotkeys` and `bindings` compatibility.

## Phase 3: Capability and Adapter Boundaries

- [x] Include motion observation, consumption, and synthesized input in capability planning.
- [x] Keep real mouse click synthesis diagnosable when unsupported by the current portal path.
- [x] Add a testable motion runtime for sequence matching, held-state tracking, repeat activation, and cancellation.
- [x] Route scripted runner lifecycle motion events through macro execution and repeat ticks.
- [x] Fail closed before real runner activation for motions when input observation is unavailable.
- [x] Emit mouse click macro actions through the RemoteDesktop portal pointer-button API.
- [x] Document the KDE/KWin motion provider contract for the remaining compositor-side work.
- [x] Add explicit unsafe evdev observation provider for configured input devices.
- [x] Poll real provider motion events and schedule repeat ticks in the live runner.
- [x] Implement evdev grab/consume mode.
- [x] Implement uinput output backend.
- [ ] Implement real KDE motion sequence observation.

## Phase 4: Documentation and Verification

- [x] Update README and LuaLS metadata.
- [x] Add example motion script.
- [x] Add Spec Kit artifacts for the feature.
- [ ] Record manual KDE Wayland verification when a real provider exists.
