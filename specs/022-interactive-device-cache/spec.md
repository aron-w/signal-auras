# Feature Specification: Interactive Device Cache

**Feature Branch**: `022-interactive-device-cache`

**Created**: 2026-06-17

**Status**: Draft

**Input**: User description: "Implement mandatory interactive evdev device selection caching for every canonical main Lua path under `$XDG_RUNTIME_DIR/signal-auras/input-devices/`. `examples/poe2.lua` should be able to configure interactive startup device selection, reuse a valid cache, validate that cached `/dev/input/event*` paths still refer to the same hardware and still have permissions, prompt with a terminal checklist when needed, request selected-device ACLs like `just unsafe-input-acl` but scoped to the interactive startup phase, cache the selection, then start."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reuse Valid Script Device Cache (Priority: P1)

A user starts a Lua automation script that uses interactive evdev selection and expects the runner to reuse the previously selected devices without prompting when the cached devices are still the same hardware and permissions are still available.

**Why this priority**: Daily startup must be quick and predictable while still avoiding unsafe reuse of stale `/dev/input/event*` paths.

**Independent Test**: Start a script with a valid existing runtime cache for its canonical main Lua path; verify startup uses only the cached selected devices, does not prompt, and preserves existing strict selected-device behavior.

**Acceptance Scenarios**:

1. **Given** a Lua script configured for interactive evdev selection and a valid runtime cache for its canonical main path, **When** the runner starts, **Then** the cached selected devices are used as explicit selected devices and startup continues without prompting.
2. **Given** the same cached hardware is reachable through stable `/dev/input/by-signal-auras/...` paths, **When** the runner validates the cache, **Then** those stable paths remain preferred and the runner does not broaden selection to unrelated devices.
3. **Given** the cache points to the runner's own virtual output device, **When** startup validates the cache, **Then** the cache is rejected and the user is sent through interactive selection or a fail-closed diagnostic.

---

### User Story 2 - Select Devices Interactively on First Startup (Priority: P2)

A user starts `examples/poe2.lua` or another main Lua script for the first time and can select the intended keyboard and pointer devices from a terminal checklist before automation begins.

**Why this priority**: The feature must remove hard-coded local event paths from examples while keeping unsafe input observation explicit and user controlled.

**Independent Test**: Start an interactive-device script with no runtime cache in an interactive terminal; verify eligible devices are listed, the user can select one or more devices, selected-device permissions are checked, the cache is created for the canonical main Lua path, and the runner starts with those selected devices.

**Acceptance Scenarios**:

1. **Given** no runtime cache exists for the canonical main Lua path, **When** the user starts the script in an interactive terminal, **Then** the runner displays a checklist of eligible local input devices before opening them for live observation.
2. **Given** the user selects devices and required permissions are already present, **When** the selection is accepted, **Then** the runner writes the runtime cache and starts with only the selected devices.
3. **Given** the user cancels the checklist, **When** startup resumes, **Then** the runner fails closed without observing input, creating a virtual output device, or writing a successful cache.

---

### User Story 3 - Repair Missing Permissions During Startup (Priority: P3)

A user with missing evdev or uinput access can authorize a temporary permission repair for the selected devices during the interactive startup phase.

**Why this priority**: The existing temporary ACL workflow is useful for local testing, but interactive startup must target only the selected devices instead of granting broad access to all event devices.

**Independent Test**: Simulate selected devices without readable evdev access and a configured uinput output without read/write access; verify the startup flow offers a selected-device permission repair, runs only after explicit confirmation, revalidates the selected devices, then starts or fails closed.

**Acceptance Scenarios**:

1. **Given** selected devices are missing read access, **When** the user confirms permission repair, **Then** the runner requests a temporary ACL grant only for the selected evdev paths.
2. **Given** uinput output is configured and `/dev/uinput` lacks read/write access, **When** the user confirms permission repair, **Then** the runner requests temporary access for `/dev/uinput` and revalidates before starting.
3. **Given** the permission repair is denied, fails, or is unavailable, **When** startup continues, **Then** the runner fails closed with actionable remediation and no hidden privilege escalation.

---

### User Story 4 - Diagnose Cache and Device State Safely (Priority: P4)

An operator can inspect why an interactive device cache is accepted or rejected without changing permissions or starting input observation.

**Why this priority**: Cache invalidation and Linux input permissions are difficult to understand from raw device paths; safe diagnostics reduce trial-and-error with privileged helpers.

**Independent Test**: Run input diagnostics against an interactive-device script; verify the report includes the runtime cache path, cache presence, validation status, selected device identities, permission status, and remediation without granting permissions or rewriting the cache.

**Acceptance Scenarios**:

1. **Given** diagnostics are requested for a script with an interactive cache, **When** the report is generated, **Then** it shows the cache path derived from the canonical main Lua path and whether each cached device is valid.
2. **Given** a cached `/dev/input/event*` path now refers to different hardware, **When** diagnostics run, **Then** the report marks the cache stale and explains that startup will require interactive selection.
3. **Given** the runtime cache is missing, **When** diagnostics run, **Then** the report explains that the next interactive startup will create it and that non-interactive startup will fail closed.

### Edge Cases

- `$XDG_RUNTIME_DIR` is missing, empty, inaccessible, or not owned by the current user.
- The same main Lua file is invoked through different relative paths, symlinks, or working directories.
- The runtime cache disappears between validation and provider startup.
- A cached `/dev/input/event*` path is reused by a different physical device after reboot, logout, replug, dock switch, or KVM switch.
- A cached stable symlink now resolves to a different event node or no longer exists.
- A selected device is unplugged, becomes unreadable, loses grab permission, or is replaced while the prompt is open.
- The selected device set is empty, contains duplicates, contains unsupported devices, or contains the runner's own virtual output device.
- Permission repair succeeds for some selected paths but not all required paths.
- Startup is non-interactive and the cache is missing, stale, or permission-incomplete.
- Multiple scripts using interactive selection start in the same user session and must not share device selections unless they have the same canonical main Lua path.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support an explicit interactive evdev device selection mode for Lua input providers.
- **FR-002**: System MUST create, validate, and use a runtime device-selection cache for every canonical main Lua path that uses interactive selection.
- **FR-003**: System MUST store interactive device caches under `$XDG_RUNTIME_DIR/signal-auras/input-devices/` and MUST fail closed with remediation when the runtime directory is unavailable or unsafe.
- **FR-004**: System MUST derive cache identity from the resolved canonical main Lua path so different main scripts do not accidentally share selected devices.
- **FR-005**: System MUST treat the runtime cache as mandatory for interactive selection but volatile across login sessions, reboots, and runtime-directory cleanup.
- **FR-006**: System MUST validate cached device paths against current hardware identity before using them.
- **FR-007**: System MUST reject a cached `/dev/input/event*` path when it no longer refers to the same hardware identity recorded when the cache was created.
- **FR-008**: System MUST validate selected-device read access, uinput read/write access when configured, duplicate paths, selected stable symlink targets, unsupported devices, grab suitability, and own-virtual-device exclusion before accepting a cache.
- **FR-009**: System MUST resolve a valid interactive cache into the same strict selected-device behavior used by explicit configured device paths.
- **FR-010**: System MUST display an interactive terminal checklist when the cache is missing, stale, invalid, incomplete, or permission-incomplete and the startup session is interactive.
- **FR-011**: System MUST fail closed without observing input, grabbing devices, emitting input, or writing a successful cache when startup is non-interactive and the cache cannot be validated.
- **FR-012**: System MUST allow users to cancel interactive selection without side effects beyond diagnostics.
- **FR-013**: System MUST request temporary permission repair only after explicit user confirmation and only for the selected evdev paths plus `/dev/uinput` when required.
- **FR-014**: System MUST revalidate selected devices after any permission repair before writing or accepting the cache.
- **FR-015**: System MUST avoid broad all-event-device permission repair during interactive startup unless the user runs a separate documented manual fallback outside this flow.
- **FR-016**: System MUST update `examples/poe2.lua` so its evdev device configuration can use interactive selection rather than hard-coded local event paths.
- **FR-017**: System MUST keep existing `devices = "all"` behavior separate from interactive selection and MUST NOT persist broad discovered devices as an interactive cache.
- **FR-018**: System MUST provide diagnostics that show runtime cache path, cache presence, validation result, selected device status, permission status, stable-path recommendations, and remediation without changing permissions or cache contents.
- **FR-019**: System MUST preserve explicit unsafe evdev/uinput consent boundaries, visible configuration, current-run grabs, current-user scoped permission behavior, and revocation by deleting runtime cache or removing session ACLs.
- **FR-020**: System MUST define the standalone Rust behavior for cache identity, cache validation, prompt decisions, and permission repair outcomes before CLI, Lua, or desktop integration behavior.
- **FR-021**: System MUST define Wayland/KDE Plasma assumptions and unsupported portal cases for interactive device selection.
- **FR-022**: System MUST ensure Lua scripts receive no ambient access to cache files, device handles, permission helpers, raw device identity probes, or broader input authority.
- **FR-023**: System MUST include automated coverage for cache key derivation, stale hardware detection, prompt decisions, selected-device permission repair, non-interactive failure, diagnostics, and PoE2 example loading.

### Key Entities

- **Interactive Device Selection**: A Lua-declared startup mode that asks the current user to select evdev devices when the per-script runtime cache cannot be safely reused.
- **Canonical Main Lua Path**: The resolved main script path used to derive the mandatory runtime cache identity.
- **Device Identity Fingerprint**: The recorded hardware identity used to decide whether a current event path still represents the same device selected by the user.
- **Runtime Device Cache Entry**: The volatile per-script cache record containing selected device paths, device identities, provider mode, output mode, validation metadata, and creation/update timestamps.
- **Cache Validation Result**: The accepted, missing, stale, invalid, permission-incomplete, unsafe-runtime-dir, or cancelled status used to choose startup behavior.
- **Permission Repair Attempt**: A user-confirmed temporary permission grant request scoped to selected evdev paths and `/dev/uinput` when configured.
- **Interactive Device Diagnostic Report**: A read-only report explaining cache path, selected device state, permission state, and remediation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Valid-cache startup tests show 100% reuse without prompting and with zero silent fallback to unrelated devices.
- **SC-002**: Stale-cache tests reject reused `/dev/input/event*` paths when the current hardware identity differs from the cached identity.
- **SC-003**: First-run interactive tests create a runtime cache and start with only the user-selected devices in 100% of successful selections.
- **SC-004**: Non-interactive tests with missing, stale, invalid, or permission-incomplete caches fail closed before input observation or output creation.
- **SC-005**: Permission repair tests show only selected evdev paths and `/dev/uinput` are targeted by the startup repair flow.
- **SC-006**: Diagnostic tests report cache path, cache status, selected device status, permission status, stable-path recommendations, and remediation without modifying permissions or cache state.
- **SC-007**: PoE2 example validation shows the example can load with interactive device selection and no hard-coded local `/dev/input/event*` paths.
- **SC-008**: Feature verification passes with documented Nix commands or records unavailable Nix checks with exact failure output.
- **SC-009**: Security tests show Lua code cannot read, write, or alter interactive cache or permission-helper behavior through ambient script capabilities.

## Assumptions

- The v1 user interface for selection is a terminal checklist because KDE Plasma portals do not provide a generic immediate evdev path selection and ACL-grant flow for this backend.
- `$XDG_RUNTIME_DIR` is the required best-practice location for this mandatory runtime cache; persistent preference storage is out of scope for this feature.
- Cache entries are volatile and may be recreated frequently; this is acceptable because interactive selection remains explicit and cached validation prevents unsafe path reuse.
- Daily NixOS use should still prefer stable `/dev/input/by-signal-auras/...` paths when available, and the interactive cache should store and recommend those stable paths when they are safely resolvable.
- The selected-device permission repair flow is for high-trust local startup only and does not replace the NixOS module guidance for durable least-privilege permissions.
