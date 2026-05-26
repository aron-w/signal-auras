# Feature Specification: Wayland Compositor Adapter

**Feature Branch**: `002-wayland-compositor-adapter`

**Created**: 2026-05-26

**Status**: Draft

**Input**: User description: "real Wayland compositor adapter for global shortcuts, active process metadata, and synthesized input"

## Clarifications

### Session 2026-05-26

- Q: Which real Wayland compositor provider should this feature target first? → A: KDE Plasma Wayland first.
- Q: What must be real on KDE Plasma Wayland before this feature is complete? → A: Global shortcuts, active-process matching, and synthesized input must all work through the KDE provider.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Register Real Desktop Shortcuts (Priority: P1)

A user starts Signal Auras in a Wayland session and expects configured shortcuts to be registered with the running desktop instead of only being simulated through mocks or terminal-only flows.

**Why this priority**: Real shortcut registration is the smallest valuable slice that turns the runner from a validated configuration tool into an actual desktop automation tool.

**Independent Test**: Can be tested by starting the runner with one explicitly scoped shortcut, granting any required permission, pressing the shortcut in the desktop session, and observing that exactly the configured action is accepted for execution.

**Acceptance Scenarios**:

1. **Given** a supported Wayland session with the required shortcut capability available, **When** the user starts Signal Auras with a valid explicitly scoped shortcut, **Then** the shortcut is registered and the user sees confirmation of the effective scope and registered key combination.
2. **Given** the compositor cannot provide the required shortcut capability, **When** the user starts Signal Auras with a shortcut configuration, **Then** startup fails before registration with a clear unsupported-capability message and no hidden fallback is installed.
3. **Given** a registered shortcut is active, **When** the user stops Signal Auras, **Then** the shortcut is removed and no automation behavior remains active after shutdown.

---

### User Story 2 - Match Shortcuts Against Active Process Metadata (Priority: P2)

A user defines process-aware shortcuts and expects Signal Auras to decide whether a shortcut is active based on the currently focused application or process information exposed by the desktop session.

**Why this priority**: Process-aware rebinding is a core product promise, and real metadata is needed before users can trust application-specific bindings.

**Independent Test**: Can be tested by configuring one shortcut for a known application process, focusing matching and non-matching applications, and confirming that the shortcut only executes in the matching context.

**Acceptance Scenarios**:

1. **Given** active process metadata is available and the focused application matches the configured rule, **When** the user presses the registered shortcut, **Then** Signal Auras treats the shortcut as eligible and records the matched process identity in its runtime feedback.
2. **Given** active process metadata is available and the focused application does not match the configured rule, **When** the user presses the registered shortcut, **Then** Signal Auras ignores the shortcut and records why the rule did not match.
3. **Given** active process metadata is unavailable or permission is denied, **When** a process-scoped shortcut is configured, **Then** Signal Auras refuses to activate that shortcut and explains which metadata requirement is unavailable.

---

### User Story 3 - Execute Approved Synthesized Input (Priority: P3)

A user configures a macro that emits keys or text and expects Signal Auras to synthesize that input through the desktop session only after the required capability and consent checks pass.

**Why this priority**: Synthesized input is necessary for useful macros, but it carries higher safety risk than registration or metadata reads and must be gated accordingly.

**Independent Test**: Can be tested by running a macro that emits a short text sequence into a focused test application after explicit permission approval, then confirming the text appears only once and only while the runner is active.

**Acceptance Scenarios**:

1. **Given** synthesized input is available and the user has granted the required capability for the current run, **When** an eligible shortcut triggers a text or key macro, **Then** the input is emitted in the declared order and Signal Auras records the completed action count.
2. **Given** synthesized input permission is denied, **When** an eligible shortcut would trigger a macro, **Then** no input is emitted and the user sees a clear denied-permission message.
3. **Given** a macro is in progress, **When** the user stops Signal Auras, **Then** no further synthesized input is emitted after shutdown begins.

### Edge Cases

- The compositor advertises shortcut support but rejects a specific key combination because it is reserved or already owned.
- The active process changes between shortcut press and macro execution eligibility evaluation.
- The focused surface has no reliable process identity, such as a privileged surface, launcher, lock screen, or compositor-owned UI.
- The session exposes active application metadata but not a stable process identifier.
- Permission is granted for shortcut registration but denied for metadata or synthesized input.
- The compositor session disappears, restarts, or invalidates capabilities while Signal Auras is running.
- Multiple configured shortcuts attempt to synthesize input at nearly the same time.
- A global shortcut is configured without explicit user-visible global consent.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect whether the current Wayland desktop session can support global shortcut registration before registering any shortcut.
- **FR-002**: System MUST register configured shortcuts with the desktop session only after the shortcut scope and required permissions have been resolved for the current run.
- **FR-003**: System MUST fail before activation with a diagnosable message when required shortcut registration support is unavailable, denied, or only partially available.
- **FR-004**: System MUST unregister all shortcuts and stop accepting shortcut events when the user exits, permission is revoked, or the desktop session invalidates the registration.
- **FR-005**: System MUST obtain active application or process metadata for process-scoped shortcuts when the desktop session makes that metadata available.
- **FR-006**: System MUST expose enough active-process identity for matching and audit feedback, including a user-visible application or process name and any stronger identity available from the session.
- **FR-007**: System MUST treat missing, ambiguous, denied, or stale active-process metadata as a non-match for process-scoped shortcuts and explain the reason.
- **FR-008**: System MUST evaluate shortcut eligibility against the active process at the time a shortcut event is handled.
- **FR-009**: System MUST synthesize key and text input only after the user has explicitly granted the required current-run capability.
- **FR-010**: System MUST refuse synthesized input when the desktop session cannot provide it, permission is denied, or the focused target is unsafe or unavailable.
- **FR-011**: System MUST execute synthesized macro actions in their declared order and prevent overlapping synthesized input from the same shortcut.
- **FR-012**: System MUST provide user-visible diagnostics for each sensitive capability: shortcut registration, active-process metadata, and synthesized input.
- **FR-013**: System MUST keep defaults least-privilege: no implicit global registration, process inspection, or synthesized input may occur without explicit configuration and consent.
- **FR-014**: System MUST provide a revocation path that stops registered shortcuts, active metadata use, and synthesized input for the current run.
- **FR-015**: System MUST define the standalone automation behavior separately from any command-line, scripting, or desktop-session integration behavior so it can be verified without a live compositor.
- **FR-016**: System MUST document supported and unsupported compositor capabilities, required permissions, and expected failure modes in user-facing verification guidance.
- **FR-017**: System MUST preserve Lua script capability boundaries by exposing only approved shortcut, metadata, and synthesized-input outcomes to scripts or script-driven macros.
- **FR-018**: System MUST record runtime counts for registered shortcuts, ignored shortcut events, matched process-scoped events, denied capability attempts, completed macro actions, and shutdown cleanup.
- **FR-019**: The first real desktop provider MUST target KDE Plasma Wayland on NixOS and MUST fail explicitly on non-KDE sessions or KDE sessions lacking the required KDE/portal capabilities.
- **FR-020**: The KDE Plasma Wayland provider MUST implement real desktop-wide global shortcut registration, real active-process scoped matching, and real synthesized key/text input before this feature can be considered complete.

### Key Entities

- **Compositor Capability**: A desktop-session capability needed by Signal Auras, such as shortcut registration, active-process metadata, or synthesized input; includes availability, permission state, denial reason, and revocation state.
- **Shortcut Registration**: A current-run binding between a key combination, an effective scope, and the desktop session; includes registration status, rejection reason, and cleanup state.
- **Active Process Context**: The metadata available for the currently focused application or surface; includes user-visible name, available process identity, confidence, and freshness.
- **Synthesized Input Request**: A macro action that asks the desktop session to emit key or text input; includes requested action, consent status, target eligibility, order, and completion result.
- **Adapter Diagnostic**: A user-visible explanation of capability availability, permission outcome, rejected operations, ignored shortcut events, and cleanup results.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a supported Wayland session, a valid explicitly scoped shortcut can be registered and confirmed within 2 seconds after startup permission decisions are complete.
- **SC-002**: 100% of unsupported, denied, or partially available compositor capability cases fail before hidden desktop automation behavior is installed.
- **SC-003**: Process-scoped shortcuts execute only when the active process context matches the configured rule in at least 95% of manual verification runs across supported desktop scenarios.
- **SC-004**: Denied or unavailable synthesized-input permission results in zero emitted input and a diagnosable user-visible message in every verification run.
- **SC-005**: Shutdown cleanup removes all current-run shortcut registrations and stops further synthesized input within 1 second of user exit in supported sessions.
- **SC-006**: Runtime feedback includes counts for registrations, ignored events, matched events, denied attempts, completed macro actions, and cleanup outcomes for every completed run.
- **SC-007**: Each priority user story passes an independent verification path, with automated verification for non-desktop behavior and documented manual verification for live compositor behavior.
- **SC-008**: Feature verification is reproducible on NixOS using documented commands and any documented manual desktop-session procedure.
- **SC-009**: The feature is not complete until the KDE Plasma Wayland manual verification path demonstrates desktop-wide shortcut registration, active-process scoped matching, synthesized-input success, and shutdown cleanup on a real KDE session.

## Assumptions

- The first real adapter targets KDE Plasma Wayland sessions on NixOS and does not add X11 support.
- The feature extends the existing terminal-started runner model and does not introduce a daemon, autostart entry, persistent background service, or hidden global state.
- Permission decisions are current-run only unless a future specification defines persistent grants.
- Supported compositor behavior is capability-based: Signal Auras reports what the session can provide rather than pretending all Wayland compositors behave identically.
- Process matching continues to use the user-visible process or application naming model from the initial Lua hotkey runner unless stronger metadata is available.
- Pixel checks, image checks, window queries beyond active-process metadata, and persistent configuration storage remain out of scope for this feature.
