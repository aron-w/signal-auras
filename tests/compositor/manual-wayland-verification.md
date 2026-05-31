# Manual KDE Plasma Wayland Verification

Automated compositor coverage is not available in this repository yet. Use this
procedure in a real NixOS KDE Plasma Wayland session for the v1 runner.

## Selected Adapter Support

The first real provider target is KDE Plasma Wayland. The feature is not
complete until this file records a successful manual run for all three desktop
capabilities:

- desktop-wide global shortcut registration and event delivery
- active-process scoped match and non-match decisions from KDE/KWin metadata
- synthesized key/text input through the KDE/portal path

Non-KDE sessions, X11 sessions, missing KWin services, missing portal support,
permission denial, reserved shortcuts, and provider invalidation must fail
closed with diagnosable output.

## Environment Baseline

Record before the run:

- Date:
- Machine/session:
- KDE Plasma version:
- KWin session type:
- xdg-desktop-portal-kde status:
- Signal Auras command:

## Scoped Script Procedure

1. Enter the development shell with `nix develop`.
2. Start a KDE Plasma Wayland session.
3. Run `cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua` or a dedicated KDE test script.
4. Confirm startup output shows the Lua file, validation, effective scope, KDE provider selection, required capability probe, bridge setup if needed, and per-hotkey registration result.
5. Focus an application matching the configured process name.
6. Press the shortcut and confirm one shortcut event is reported.
7. Confirm the active-process decision is logged as a match.
8. Confirm key/text macro actions are emitted in declared order.
9. Focus a non-matching application.
10. Press the shortcut and confirm no input is emitted and the non-match reason is logged.
11. Press Ctrl-C if the process is still running.
12. Confirm hotkey cleanup, portal session cleanup, KDE bridge unload, pending input cancellation, and final stats output.

## Prompt Scope Procedure

1. Run `cargo run -p signal-auras-cli -- run ./examples/prompt-scope.lua`.
2. Confirm the terminal prints the missing-scope prompt before registration.
3. Select `1`, enter one or more comma-separated process names, and confirm the effective scope is logged for the current run.
4. Run the same command again, select `2`, type `GLOBAL`, and confirm explicit global selection is required before registration.
5. Run the same command again, select `3`, and confirm the runner exits without registering hotkeys.

## Capability Failure Verification

1. Run from a non-KDE Wayland session if available and confirm unsupported-provider failure before registration.
2. Run from an X11 session if available and confirm unsupported-session failure before registration.
3. Disable or remove xdg-desktop-portal-kde from the test session if practical and confirm synthesized-input capability failure before macro execution.
4. Use a reserved or already-owned hotkey and confirm any prior registrations are cleaned up.
5. Deny synthesized-input permission if KDE/portal offers a prompt and confirm zero input is emitted.
6. Stop or invalidate KWin/portal during a run if practical and confirm cleanup before exit.

## Global Shortcut Verification

1. Configure a single KDE-supported hotkey that is not already reserved by Plasma.
2. Start the runner and confirm `provider selected=kde-plasma-wayland` appears before registration.
3. Confirm the hotkey registration output includes the configured key and a KDE provider handle.
4. Trigger the hotkey from a focused application outside the terminal and confirm exactly one event is reported.
5. Repeat with a reserved or already-owned hotkey and confirm startup exits after cleaning up any earlier handles.
6. Press Ctrl-C and confirm no shortcut remains active after shutdown.

## Active Process Metadata Verification

1. Configure a process-scoped shortcut for a visible KDE application such as Kate.
2. Focus that application and press the registered shortcut.
3. Confirm the runner logs an active-process match with the visible process or application identity.
4. Focus a different application and press the same shortcut.
5. Confirm the runner logs a non-match and emits no macro input.
6. Repeat on a privileged or compositor-owned surface such as a lock screen or launcher when practical and confirm it is treated as unavailable or ambiguous.
7. Invalidate or deny the metadata path when practical and confirm startup or event handling fails closed with a KWin diagnostic.
8. When practical, delay or interrupt active-process metadata updates during a focus change and confirm the trigger is denied with `denial_reason=stale_focus` or `denial_reason=focus_unavailable`.
9. Confirm stale-focus diagnostics include `configured_rule`, `metadata_age_ms`, and `stale_threshold_ms=2000`, and do not include command-line arguments or window title text.

## Synthesized Input Verification

1. Configure a macro that emits short ASCII text into a focused KDE text editor.
2. Start the runner in KDE Plasma Wayland and grant any portal permission prompt.
3. Press the registered shortcut and confirm the text appears once in declared order.
4. Repeat with portal permission denied and confirm zero input is emitted.
5. Repeat with text that the key-translation path cannot represent and confirm no partial text is emitted.
6. Press Ctrl-C during or immediately after a macro and confirm portal input is cancelled before exit.

## Robust Evdev Device Selection Verification

1. Configure `input_provider.devices` with selected stable `/dev/input/by-signal-auras/...` paths and confirm only those selected devices are observed.
2. Add a missing selected path and confirm startup fails closed with an evdev diagnostic and remediation rather than broadening to other devices.
3. Run `cargo run -p signal-auras-cli -- doctor input ./path/to/script.lua` and confirm selected paths, duplicate paths, `/dev/uinput`, stable-path recommendations, and permission remediation are reported without starting input observation.
4. Switch the script to `devices = "all"` and confirm unreadable or self-generated devices are skipped, at least one eligible readable device allows startup, and no discovered paths persist after exit.
5. Unplug and replug a selected stable device during the current run and confirm the runner reports removal/reopen and resumes observing only that selected path.

## Results

### Run 2026-05-26: KDE Plasma Wayland live smoke test

- Date: 2026-05-26
- Machine/session: NixOS KDE Plasma Wayland, `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=KDE`, `WAYLAND_DISPLAY=wayland-0`
- KDE Plasma version: `plasmashell 6.6.5`
- KWin session type: `kwin 6.6.5` on Wayland
- xdg-desktop-portal-kde status: `org.freedesktop.portal.Desktop` and `org.freedesktop.impl.portal.desktop.kde` present on the user D-Bus; `xdg-desktop-portal.service` active
- Signal Auras command: `timeout -s INT 5 cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua`

Observed output:

```text
startup script_path=./examples/poe2-hideout.lua
script_validation result=ok
effective_scope processes: poe2.exe
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-F5
final_summary reason=CtrlC elapsed_ms=4156 triggers=0 successes=0 failures=0 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=0 active_process_non_matches=0 metadata_unavailable=0 input_emitted=0 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- KDE provider selected: PASS. The live session probe selected `kde-plasma-wayland`.
- Live service probe: PASS. The runner now probes KWin, KGlobalAccel, the desktop portal, and the KDE portal implementation through the user D-Bus instead of requiring test-only environment flags.
- Global shortcut registration: FAIL. The registration output still reports an internal owned handle (`kde-F5`) but does not install a real KGlobalAccel/KWin shortcut.
- Shortcut event delivery: FAIL. No desktop-wide shortcut event was delivered during the live run.
- Active-process match: NOT RUN. This depends on real shortcut event delivery and a live KWin active-window metadata bridge.
- Active-process non-match: NOT RUN. This depends on real shortcut event delivery and a live KWin active-window metadata bridge.
- Synthesized input success: NOT RUN. This depends on a real trigger path and RemoteDesktop portal emission.
- Denied synthesized input emits zero input: NOT RUN. This requires exercising the real portal permission prompt after the input path emits through the portal.
- Ctrl-C cleanup: PARTIAL. The process stopped on SIGINT and printed final stats, but no real KDE bridge, KGlobalAccel registration, or portal session cleanup occurred because those live resources were not created.
- Unsupported-session diagnostics: NOT RUN in this KDE session.
- Notes: T062 remains incomplete. The next implementation step is a current-run KDE bridge that loads a temporary KWin script using `org.kde.kwin.Scripting`, registers shortcuts via KWin/KGlobalAccel, forwards trigger and active-window metadata back to the Rust runner over a current-run D-Bus object, and unloads the script plus D-Bus object during shutdown.

### Run 2026-05-26: current-run KWin shortcut bridge smoke test

- Signal Auras command: `timeout -s INT 5 cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua`
- Registered handle: `kde-kwin-script:signal-auras-3611656-1:F5`
- KGlobalAccel cleanup check: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos | rg 'SignalAuras' || true`

Observed output:

```text
startup script_path=./examples/poe2-hideout.lua
script_validation result=ok
effective_scope processes: poe2.exe
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-3611656-1:F5
final_summary reason=CtrlC elapsed_ms=4038 triggers=0 successes=0 failures=0 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=0 active_process_non_matches=0 metadata_unavailable=0 input_emitted=0 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- Global shortcut registration: PARTIAL PASS. The runner now loads a temporary KWin script and KGlobalAccel shows a real `SignalAuras_*` shortcut while the runner is active.
- Ctrl-C cleanup: PASS for KGlobalAccel residue. After shutdown, `rg 'SignalAuras'` over KWin shortcut infos returns no entries.
- Shortcut event delivery: NOT VERIFIED. Programmatic `invokeShortcut` did not emit a runner event through the KGlobalAccel signal listener in this session; physical keypress verification is still required, or the bridge needs a Rust D-Bus callback service backed by an async runtime.
- Notes: T062 remains incomplete until desktop keypress event delivery, active-process decisions, and portal input are verified end-to-end.

### Run 2026-05-26: KWin callback event and active-process non-match

- Signal Auras command: `cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua`
- Trigger command: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component invokeShortcut s SignalAuras_4001005_1`
- Cleanup check: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos | rg 'SignalAuras' || true`

Observed output:

```text
startup script_path=./examples/poe2-hideout.lua
script_validation result=ok
effective_scope processes: poe2.exe
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-4001005-1:F5
denied_trigger hotkey=F5 reason=active process 'rustdesk' is outside configured scope
final_summary reason=CtrlC elapsed_ms=42753 triggers=1 successes=0 failures=0 denials=1 permission_failures=0 scope_mismatches=1 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=0 active_process_non_matches=1 metadata_unavailable=0 input_emitted=0 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- Shortcut event delivery: PASS for KWin callback delivery. The KWin script invoked the current-run Rust D-Bus callback and the runner counted one trigger.
- Active-process non-match: PASS. KWin supplied active-window metadata and the runner denied the scoped macro because `rustdesk` did not match `poe2.exe`.
- Ctrl-C cleanup: PASS for KGlobalAccel residue. The cleanup check returned no `SignalAuras` entries.
- Remaining T062 gaps: active-process match with a matching app, real synthesized input emission through the RemoteDesktop portal, denied synthesized-input zero-emission behavior, and physical desktop keypress verification.

### Run 2026-05-26: KWin callback active-process match

- Signal Auras command: `cargo run -p signal-auras-cli -- run /tmp/signal-auras-kde-match.lua`
- Temporary script scope: `rustdesk`
- Trigger commands:
  - `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component invokeShortcut s SignalAuras_2217918_1`
  - `busctl --user call org.signalAuras.Runner2217918 /org/signalAuras/Runner org.signalAuras.KWinBridge triggered sssss SignalAuras_2217918_1 RustDesk rustdesk rustdesk 1004576`
- Cleanup check: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos | rg 'SignalAuras' || true`

Observed output:

```text
startup script_path=/tmp/signal-auras-kde-match.lua
script_validation result=ok
effective_scope processes: rustdesk
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-2217918-1:F5
final_summary reason=CtrlC elapsed_ms=90757 triggers=2 successes=2 failures=0 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=2 active_process_non_matches=0 metadata_unavailable=0 input_emitted=0 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- Active-process match: PASS. KWin callback metadata matched the scoped `rustdesk` process.
- Shortcut event delivery: PASS for KWin callback delivery through the current-run D-Bus callback.
- Ctrl-C cleanup: PASS for KGlobalAccel residue. The cleanup check returned no `SignalAuras` entries.
- Physical desktop keypress: NOT VERIFIED. No physical `F5` trigger was observed during the live waiting window; only D-Bus-triggered events were counted.

### Run 2026-05-26: RemoteDesktop portal synthesized input

- Signal Auras command: `cargo run -p signal-auras-cli -- run /tmp/signal-auras-kde-input.lua`
- Temporary script scope: `rustdesk`
- Temporary script macro: `text "sa"`
- Trigger command: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component invokeShortcut s SignalAuras_2287141_1`
- Cleanup check: `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos | rg 'SignalAuras' || true`

Observed output:

```text
startup script_path=/tmp/signal-auras-kde-input.lua
script_validation result=ok
effective_scope processes: rustdesk
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-2287141-1:F5
final_summary reason=CtrlC elapsed_ms=127963 triggers=1 successes=1 failures=0 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=1 active_process_non_matches=0 metadata_unavailable=0 input_emitted=1 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- Synthesized input success: PASS. The live runner opened the KDE RemoteDesktop portal path and reported one emitted input request.
- Active-process match: PASS. The portal input was emitted only after the KWin callback reported a matching active process.
- Ctrl-C cleanup: PASS for KGlobalAccel residue. The cleanup check returned no `SignalAuras` entries.
- Physical desktop keypress: NOT VERIFIED in this run. The observed portal input trigger used KGlobalAccel D-Bus invocation, not a hardware keypress.
- Denied synthesized input emits zero input: NOT VERIFIED in this run. The KDE portal permission-denial path was not exercised in this session.

### Run 2026-05-26: physical F5 with denied RemoteDesktop portal

- Signal Auras command: `just run-prompt`
- Script: `examples/prompt-scope.lua`
- Prompt choices: `2`, then `GLOBAL`
- Trigger: physical `F5` keypress in the KDE Plasma Wayland session
- Portal response: RemoteDesktop permission denied after revoking `kde-authorized remote-desktop` with the portal permission store

Observed output:

```text
startup script_path=examples/prompt-scope.lua
script_validation result=ok
effective_scope global (explicit current run)
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-1162050-1:F5
final_summary reason=RuntimeError elapsed_ms=4422 triggers=1 successes=0 failures=1 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=1 active_process_non_matches=0 metadata_unavailable=0 input_emitted=0 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
error capability_probe: required permission was denied (capability: synthesized_input) remediation: grant the requested permission and restart the runner source: xdg-desktop-portal RemoteDesktop
```

- Physical desktop keypress: PASS. A hardware `F5` press reached the current-run KWin shortcut bridge.
- Denied synthesized input emits zero input: PASS. The denied portal request produced `input_emitted=0` and a diagnosable synthesized-input permission error.

### Run 2026-05-26: physical F5 with granted RemoteDesktop portal

- Signal Auras command: `just run-prompt`
- Script: `examples/prompt-scope.lua`
- Prompt choices: `2`, then `GLOBAL`
- Trigger: physical `F5` keypress in the KDE Plasma Wayland session
- Portal response: RemoteDesktop permission granted

Observed output:

```text
startup script_path=examples/prompt-scope.lua
script_validation result=ok
effective_scope global (explicit current run)
provider selected=kde-plasma-wayland
capability_probe result=ok
hotkey_registered hotkey=F5 id=kde-kwin-script:signal-auras-1169284-1:F5
/hideout
final_summary reason=CtrlC elapsed_ms=46238 triggers=1 successes=1 failures=0 denials=0 permission_failures=0 scope_mismatches=0 capability_probe_successes=1 capability_probe_failures=0 ignored_events=0 active_process_matches=1 active_process_non_matches=0 metadata_unavailable=0 input_emitted=3 input_denied=0 kde_bridge_setups=0 kde_bridge_cleanups=0 cleanup_successes=0 cleanup_failures=0
```

- Physical desktop keypress: PASS. A hardware `F5` press triggered the macro and emitted `/hideout`.
- Synthesized input success: PASS. The macro emitted three ordered input requests: `Enter`, text, `Enter`.
- Ctrl-C cleanup: PASS. Shutdown completed after SIGINT and left no reported cleanup failures.
- T062 status: PASS. All required KDE Plasma Wayland manual compositor scenarios have successful recorded evidence.

### Follow-up: callback wakeup latency and idle efficiency

After implementing callback wake fds, repeat the physical F5 checks with
`--verbose` enabled and confirm:

- A physical shortcut press while the runner is otherwise idle logs
  `callback_received` with `dispatch_latency_ms` and dispatches the macro
  without waiting for keyboard, pointer, repeat, or shutdown activity.
- A short burst of physical or KGlobalAccel-invoked F5 callbacks produces one
  callback disposition per accepted event and logs `callback_burst_limited`
  rather than silently losing events if the queue limit is reached.
- Leaving the runner idle produces no repetitive idle diagnostics, and the next
  callback still dispatches promptly.
- Pressing Ctrl-C while callbacks are pending exits without starting new
  callback-triggered macro work after shutdown begins.

### Follow-up: runtime shutdown reliability

After implementing runtime shutdown reliability, repeat live runner shutdown
checks with configured shortcuts and unsafe input when available:

- Start `signal-auras run --verbose <lua-file>` with KDE callbacks enabled,
  press Ctrl-C, and confirm final stats print before current-run shortcut,
  KWin bridge, virtual input, and grab cleanup completes.
- Send SIGTERM to the runner process and confirm it exits through the same
  runtime shutdown path with `reason=SignalTerm` rather than abrupt
  termination.
- While a motion grab/uinput config is active, stop the runner and confirm the
  physical device is usable immediately afterward and no Signal Auras virtual
  input device remains stuck.

### Follow-up: true input latency metrics

After implementing true input latency metrics, repeat unsafe evdev motion
checks with `--verbose` enabled and confirm:

- Motion input diagnostics include `dispatch_after_read_latency_ms` and
  `event_age_ms` as separate fields.
- Final stats include `motion_event_age_samples`,
  `motion_event_age_unavailable`, `avg_motion_event_age_ms`,
  `p95_motion_event_age_ms`, `p99_motion_event_age_ms`, and
  `max_motion_event_age_ms`.
- If the selected evdev source does not provide comparable kernel timestamps,
  the runner continues and increments unavailable event-age samples while
  dispatch-after-read latency remains available.

### Follow-up: full keyboard key coverage

After implementing full keyboard key coverage, repeat unsafe evdev/uinput
checks with a selected keyboard path and confirm:

- `signal-auras doctor keys <lua-file>` reports current-run device status, raw
  key code, canonical token, aliases, triggerability, emittability, and
  unavailable reasons without persisting observed keys.
- Representative Keychron K5 Pro letter, number, punctuation, function,
  navigation, keypad, and media keys produce canonical names.
- Hardware-only Fn/layer/firmware controls that emit no Linux input event are
  reported as unobserved rather than guessed.
- Macros can emit supported expanded keys through `/dev/uinput`; unsupported
  backend keys fail closed without substituting another key.

### Follow-up: scoped focus pass-through

After implementing scoped focus pass-through, repeat process-scoped KDE checks
with a scoped hotkey, scoped motion, and scoped repeat:

- Focus outside the configured process and confirm original physical input
  reaches the focused application, no scoped macro output is emitted, and no
  scoped consumed/prevented event is reported.
- Focus the configured process and confirm subsequent scoped triggers work
  normally under the existing consent and capability rules.
- Move focus back outside the configured process while scoped repeat or delayed
  output is pending and confirm scoped queued work is cancelled before further
  output.
- Confirm activation and deactivation emit one info-level
  `scoped_focus_transition` log per state change without command-line
  arguments, window titles, text payloads, or macro payloads.

## Composite Pointer Bindings

Composite pointer bindings remain blocked on a real KDE provider for pointer
observation and event consumption. Until that provider exists, run the composite
example and verify that consumed pointer bindings fail closed before
registration:

```bash
nix develop -c cargo run -p signal-auras-cli -- run ./examples/composite-bindings.lua
```

Expected result: `capability_probe` fails with the
`composite_pointer_observation` capability and no pointer binding is registered.

When KDE pointer observation and consumption support is added, manually verify:

- `Ctrl+WheelUp` emits `Left` without zooming or scrolling.
- `Ctrl+WheelDown` emits `Right` without zooming or scrolling.
- `Ctrl+LeftClick` emits `Alt+Right`, `hello world`, and `Enter`.
- Out-of-scope applications are unaffected.
- Ctrl-C removes all current-run registrations.
