# Feature Specification: Robust Device Selection

**Feature Branch**: `011-robust-device-selection`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Evdev device selection must tolerate unreadable and noisy devices. Cover selected stable devices, `devices = \"all\"` startup with unreadable devices, hotplug and reopen behavior, own-virtual-device exclusion, and doctor diagnostics. Selected `/dev/input/by-signal-auras/...` paths are preferred for daily use."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Use Selected Stable Devices Predictably (Priority: P1)

A user configures explicit stable input device paths and expects the runner to observe only those intended devices, with clear feedback if a selected path cannot be used.

**Why this priority**: Explicit device selection is the safest daily-use mode because it limits global input observation to devices the user intentionally named.

**Independent Test**: Configure selected stable paths, including `/dev/input/by-signal-auras/...` paths; verify only selected readable eligible devices are observed and missing, unreadable, or ineligible selected paths produce actionable diagnostics without silent fallback.

**Acceptance Scenarios**:

1. **Given** a readable selected stable device path, **When** the runner starts, **Then** input from that device is eligible for configured bindings and unrelated devices are not observed.
2. **Given** a selected path is missing, unreadable, or ineligible, **When** the runner starts, **Then** startup fails closed or marks the selected path unusable according to documented selected-device policy and reports remediation.
3. **Given** a selected path resolves to the runner's own virtual output device, **When** device selection is evaluated, **Then** that device is rejected with a diagnostic rather than observed.

---

### User Story 2 - Start With `devices = "all"` Despite Unreadable or Noisy Devices (Priority: P2)

A user opts into broad current-run device discovery and expects unreadable, unsupported, or noisy event devices to be skipped without preventing eligible devices from working.

**Why this priority**: Real desktops often expose event devices that are unreadable, irrelevant, or noisy; a single bad device should not crash an explicitly broad run.

**Independent Test**: Simulate startup with a mix of readable eligible devices, unreadable devices, unsupported devices, noisy devices, and permission-denied devices; verify eligible devices are used, skipped devices are reported, and no hidden privilege escalation occurs.

**Acceptance Scenarios**:

1. **Given** `devices = "all"` and at least one readable eligible input device, **When** other devices are unreadable or unsupported, **Then** the runner starts with eligible devices and reports skipped devices.
2. **Given** `devices = "all"` and no readable eligible devices, **When** the runner starts, **Then** it fails closed with a clear no-usable-devices diagnostic.
3. **Given** a noisy device emits unsupported events, **When** the runner is active, **Then** unsupported noise is ignored or summarized without starving eligible input.

---

### User Story 3 - Recover From Hotplug and Reopen Conditions (Priority: P3)

A user expects selected devices and broad device discovery to handle device removal, permission changes, and reappearance during the current run without silently losing all input.

**Why this priority**: Keyboards, mice, KVM switches, and desktop device paths can disappear and reappear while automation is running.

**Independent Test**: Simulate selected and `devices = "all"` devices being removed, becoming unreadable, reappearing, or changing eligibility; verify the runner reports state changes and reopens eligible devices according to the configured selection mode.

**Acceptance Scenarios**:

1. **Given** a selected stable path disappears, **When** it later reappears as a readable eligible device, **Then** the runner reports the transition and resumes observing that selected path without broadening to unrelated devices.
2. **Given** `devices = "all"` is active, **When** a new readable eligible device appears, **Then** it becomes eligible during the current run and skipped devices remain diagnosable.
3. **Given** a device becomes unreadable after startup, **When** the runner detects the failure, **Then** it marks that device inactive, reports the reason, and keeps remaining eligible devices active.

---

### User Story 4 - Diagnose Device Selection Safely (Priority: P4)

An operator uses doctor diagnostics to understand which devices are eligible, skipped, unreadable, noisy, self-generated, or better represented by stable paths.

**Why this priority**: Device selection failures are difficult to diagnose from raw `/dev/input` names; users need guidance that preserves explicit consent.

**Independent Test**: Run doctor diagnostics against a mixed set of selected, discovered, unreadable, noisy, and own-virtual devices; verify the output explains status, permission remediation, and stable path suggestions without enabling new observation by itself.

**Acceptance Scenarios**:

1. **Given** doctor diagnostics are requested, **When** eligible devices have stable `/dev/input/by-signal-auras/...` paths, **Then** diagnostics recommend those paths for daily selected-device configuration.
2. **Given** unreadable or permission-denied devices are present, **When** doctor diagnostics run, **Then** they report the affected paths and least-privilege remediation steps.
3. **Given** the runner's own virtual output device is present, **When** doctor diagnostics run, **Then** it is identified as excluded and not recommended for input observation.

### Edge Cases

- `devices = "all"` remains an explicit current-run opt-in and does not persist discovered devices.
- Explicit selected paths are never silently replaced by other devices with similar names.
- A device that is both noisy and eligible remains usable for supported events while unsupported noise is bounded and diagnosable.
- Unreadable devices, denied grabs, disappeared devices, duplicate paths, symlink changes, and permission revocation do not crash the runner.
- Own virtual output devices are excluded from automatic discovery and rejected if explicitly selected.
- Doctor diagnostics do not grant input observation permissions or enable hidden global behavior.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support explicit selected device paths and treat selected `/dev/input/by-signal-auras/...` paths as the preferred daily-use guidance.
- **FR-002**: System MUST observe only configured selected devices when explicit device paths are configured.
- **FR-003**: System MUST report missing, unreadable, permission-denied, duplicate, ineligible, or self-generated selected paths without silently substituting unrelated devices.
- **FR-004**: System MUST tolerate unreadable, unsupported, or noisy devices during `devices = "all"` startup by using eligible readable devices and reporting skipped devices.
- **FR-005**: System MUST fail closed with a diagnosable no-usable-devices result when no configured or discovered eligible input device can be used.
- **FR-006**: System MUST ignore or summarize unsupported noisy events without starving supported input from eligible devices.
- **FR-007**: System MUST exclude the runner's own virtual output device from automatic discovery and reject it when explicitly selected.
- **FR-008**: System MUST handle device removal, unreadable transitions, permission revocation, and reappearance during the current run according to the configured selection mode.
- **FR-009**: System MUST reopen reappearing selected stable paths without broadening explicit selection to unrelated devices.
- **FR-010**: System MUST keep `devices = "all"` hotplug discovery current-run only and avoid persisting discovered device state.
- **FR-011**: System MUST provide doctor diagnostics for selected, discovered, skipped, unreadable, noisy, duplicate, self-generated, and permission-denied devices.
- **FR-012**: System MUST include least-privilege permission remediation and stable path recommendations in doctor diagnostics.
- **FR-013**: System MUST preserve explicit unsafe evdev/uinput consent boundaries, visible configuration, current-run behavior, and revocation behavior.
- **FR-014**: System MUST include automated coverage for selected stable paths, `devices = "all"` mixed-device startup, noisy devices, hotplug and reopen behavior, own-virtual-device exclusion, doctor diagnostics, and permission failures.

### Key Entities

- **Selected Device Path**: A user-configured device path or stable symlink that intentionally scopes input observation.
- **Discovered Device Candidate**: A device considered during explicit `devices = "all"` current-run discovery.
- **Device Eligibility Result**: The readable, skipped, ineligible, unreadable, duplicate, self-generated, or permission-denied status for a device.
- **Own Virtual Output Device**: The runner-created output device that must not be re-observed as input.
- **Doctor Diagnostic Report**: A user-requested summary of device eligibility, skipped reasons, permission remediation, and stable path recommendations.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Selected-device tests show only configured selected readable devices are observed, with zero silent fallback to unrelated devices.
- **SC-002**: Mixed startup tests for `devices = "all"` with unreadable, unsupported, noisy, and readable devices start successfully when at least one eligible readable device exists.
- **SC-003**: No-usable-device tests fail closed with a diagnostic that identifies why no eligible device can be used.
- **SC-004**: Hotplug tests report removed, unreadable, reappearing, and newly eligible devices within 1 second of detection in covered scenarios.
- **SC-005**: Own-virtual-device tests show the runner excludes or rejects its own output device in 100% of covered selection modes.
- **SC-006**: Noisy-device tests show supported input from eligible devices is not starved by unsupported event noise.
- **SC-007**: Doctor diagnostic tests include device status, skipped reasons, least-privilege remediation, own-device exclusion, and `/dev/input/by-signal-auras/...` path recommendations where available.
- **SC-008**: Permission and revocation tests show no hidden privilege escalation and no persisted discovered device state.
- **SC-009**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- Unsafe evdev/uinput input remains an explicit high-trust local backend and is not enabled by default.
- Users who run daily automation are expected to prefer selected stable `/dev/input/by-signal-auras/...` paths over broad `devices = "all"` discovery.
- `devices = "all"` remains useful for discovery, troubleshooting, and temporary broad runs when the user explicitly opts in.
- Device discovery and diagnostics must be useful without adding a daemon, autostart entry, IPC endpoint, or persistent device cache.
