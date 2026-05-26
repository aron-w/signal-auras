# Research: Wayland Compositor Adapter

## Decision: Target KDE Plasma Wayland as the first real provider

**Rationale**: The clarified feature requires KDE Plasma Wayland first, with all three capabilities real before completion. KDE gives this feature a concrete compositor/session boundary: KWin owns Wayland window state, KDE exposes global shortcut infrastructure, and xdg-desktop-portal-kde participates in portal-mediated desktop automation permissions.

**Alternatives considered**:
- Keep provider-neutral Wayland skeleton: rejected because the feature would remain fail-closed without real desktop-wide behavior.
- Target wlroots/Hyprland first: rejected by user clarification.
- Treat synthesized input as optional: rejected by FR-020 and SC-009.

## Decision: Use a KDE provider facade inside `signal-auras-wayland`

**Rationale**: The existing adapter traits already separate pure core behavior from desktop side effects. A `KdePlasmaAdapter` facade can compose session detection, shortcut registration, active-window metadata, portal input, diagnostics, event dispatch, and cleanup without leaking KDE-specific types into `signal-auras-core`, Lua, or CLI contracts.

**Alternatives considered**:
- Move KDE D-Bus calls into the CLI runner: rejected because core automation behavior and adapter error mapping would become difficult to test.
- Put KDE types in `signal-auras-core`: rejected because core must remain provider-independent.
- Add a long-lived daemon: rejected because the feature is current-run only and must avoid hidden global behavior.

## Decision: Prefer D-Bus/portal crates over shelling out to KDE tools

**Rationale**: `zbus` provides typed async D-Bus integration from Rust, and `ashpd` provides xdg-desktop-portal bindings. Structured API calls allow explicit error mapping, cancellation, object lifetime tracking, and test fakes. Shell utilities would make permission, cleanup, and diagnostics less reliable.

**Alternatives considered**:
- Call `qdbus`/`gdbus` commands: rejected because parsing command output would be brittle and harder to test.
- Add C++/Qt helper binaries: rejected unless a KDE-only API proves impossible from Rust, because helpers add build and safety boundaries.
- Vendor protocol XML manually: rejected unless no maintained Rust crate or packaged protocol satisfies the requirement.

## Decision: Model KDE global shortcut delivery as owned current-run registrations

**Rationale**: Shortcut registration must be visible, scoped, and revocable. The provider should use KDE global shortcut infrastructure when it can deliver events to the current runner. If KDE requires a bridge, the bridge must be current-run only, user-visible, and removed on shutdown. Each hotkey maps to an owned handle and must clean up after partial registration failure, Ctrl-C, provider invalidation, or runtime error.

**Alternatives considered**:
- Persist KDE global shortcuts across runs: rejected because persistent registration is out of scope and would surprise users.
- Install shortcuts through desktop settings manually: rejected because it cannot be tested or cleaned up by the runner.
- Fall back to raw key capture: rejected because Wayland security does not permit generic global keyboard hooks and the constitution requires compositor-aware behavior.

## Decision: Read active-process metadata through KDE/KWin active-window state

**Rationale**: KDE Plasma Wayland centralizes active window state in KWin. The provider should convert KDE active-window metadata into `ActiveProcessContext` with visible name, app/window class when available, optional PID when available, confidence, freshness, and diagnostics. Missing, ambiguous, privileged, stale, or denied metadata remains a non-match for process-scoped shortcuts.

**Alternatives considered**:
- Inspect `/proc` globally without compositor context: rejected because it does not identify the focused Wayland surface and broad process inspection would exceed the feature.
- Reuse the last known active process when a fresh read fails: rejected because process-aware matching must be conservative.
- Match only application display names: rejected because KDE may expose stronger identity; the model should preserve it when available.

## Decision: Use xdg-desktop-portal RemoteDesktop for synthesized input

**Rationale**: Synthesized key/text input is sensitive and must go through an explicit permissioned desktop API. The RemoteDesktop portal exposes session-based keyboard notification APIs suitable for approved key emission, and xdg-desktop-portal-kde is the KDE path for portal mediation. Text macros should be translated into ordered keyboard input where the portal/provider can represent the characters; unsupported characters fail with diagnostics and emit no further input in that macro.

**Alternatives considered**:
- Use compositor-private fake-input protocols directly: rejected as the primary path because portal mediation better matches current-run consent and desktop policy.
- Use `ydotool`, `/dev/uinput`, or privileged helpers: rejected because they bypass KDE/Wayland consent and require broader privileges.
- Paste text through clipboard manipulation: rejected because clipboard mutation is outside the macro contract and introduces hidden state.

## Decision: Keep Lua API stable and capability-free

**Rationale**: Existing Lua scripts declare scopes, hotkeys, and ordered macro actions. The host decides which KDE capabilities are needed, asks for current-run consent, and reports outcomes. Lua scripts must not receive raw active-window metadata, D-Bus access, portal handles, or input injection APIs.

**Alternatives considered**:
- Add Lua APIs for querying active process: rejected because scripts would gain ambient process metadata.
- Add Lua APIs for raw input injection: rejected because macro actions already provide a safer declarative request.
- Encode KDE permissions in Lua: rejected because permissions are current-run host decisions.

## Decision: Use automated adapter-contract tests plus manual KDE verification

**Rationale**: Capability mapping, registration lifecycle, active-process decisions, input ordering, cancellation, stats, and diagnostics can be tested with fake KDE/portal clients. Real desktop-wide shortcut capture, focused-window metadata, portal permission prompts, and input emission require an interactive Plasma Wayland session until a nested KDE harness exists.

**Alternatives considered**:
- Claim completion from fake adapter tests: rejected because SC-009 requires real KDE manual verification.
- Block this feature on a nested Plasma CI harness: rejected as over-expanding this feature before a working provider exists.
- Test only happy paths manually: rejected because unsupported sessions, denied permissions, reserved shortcuts, and cleanup are release blockers.

## Decision: Update Nix and Cargo only for the selected KDE path

**Rationale**: Reproducibility requires every added Rust crate and native desktop dependency to be represented in the repository. The first likely additions are `zbus`, `ashpd`, and any native packages needed to build or manually verify KDE portal/D-Bus behavior. Broad desktop dependency sets should not be added until the implementation needs them.

**Alternatives considered**:
- Depend on host-installed KDE developer tools without Nix representation: rejected because verification must be reproducible on NixOS.
- Add all Plasma packages preemptively: rejected because it obscures what the adapter actually requires.
- Vendor KDE protocol or D-Bus definitions without package support: rejected unless upstream packaged definitions cannot be consumed.

## References

- KDE KWin scripting API: https://develop.kde.org/docs/plasma/kwin/api/
- KDE Frameworks KGlobalAccel API: https://api.kde.org/kglobalaccel.html
- xdg-desktop-portal RemoteDesktop interface: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html

