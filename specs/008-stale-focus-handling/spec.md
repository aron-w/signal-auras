# Feature Specification: Stale Focus Handling

**Feature Branch**: `008-stale-focus-handling`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Focused-process metadata must fail closed when stale so process-aware macros do not fire for the wrong focused process. Handle lost or delayed compositor metadata, produce diagnosable stale-focus denials, preserve the Lua API, and keep the default stale threshold at 2 seconds unless clarified later."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Prevent Wrong-Process Macro Execution (Priority: P1)

A user binds a macro to a specific focused process and expects the macro to run only when the runtime has current focus metadata proving that the intended process is focused.

**Why this priority**: A stale focus match can send input to the wrong application, which is the highest-impact failure this feature prevents.

**Independent Test**: Exercise a process-scoped binding with fresh matching metadata, then with the same metadata after it exceeds the stale threshold; verify the fresh attempt is eligible and the stale attempt is denied before any macro action occurs.

**Acceptance Scenarios**:

1. **Given** a process-scoped macro and fresh focused-process metadata that matches its rule, **When** the user triggers the binding, **Then** the macro is allowed to proceed under the existing consent boundary.
2. **Given** the last focused-process metadata is older than the stale threshold, **When** the user triggers a process-scoped binding, **Then** the binding is denied and no macro action is emitted.
3. **Given** stale metadata still names the previously focused allowed process, **When** another application has focus but no fresh metadata has arrived, **Then** the stale process name is not trusted and the process-scoped macro is denied.
4. **Given** the live KDE bridge has cached a matching active-process snapshot from a compositor callback, **When** that cached state is read later without a new focus callback, **Then** the snapshot keeps the original callback timestamp and becomes stale according to the configured threshold.

---

### User Story 2 - Handle Lost or Delayed Focus Metadata (Priority: P2)

A user running under KDE Plasma Wayland expects process-aware bindings to behave conservatively when compositor metadata is delayed, unavailable, or temporarily lost during focus changes.

**Why this priority**: Metadata interruptions are normal compositor conditions; the runtime must recover without making unsafe assumptions or requiring users to restart.

**Independent Test**: Simulate unavailable, delayed, stale, and recovered focus metadata while triggering process-scoped bindings; verify unknown focus denies scoped macros and fresh recovered metadata restores normal matching.

**Acceptance Scenarios**:

1. **Given** focused-process metadata is unavailable because the compositor or permission is unavailable, **When** a process-scoped binding is triggered, **Then** the binding is denied with an unavailable-focus reason.
2. **Given** metadata updates arrive after a focus change, **When** a process-scoped binding is triggered before the fresh update arrives, **Then** the binding is denied until current matching metadata is available.
3. **Given** fresh matching metadata later becomes available, **When** the same process-scoped binding is triggered again, **Then** normal process matching resumes without restarting the runner.

---

### User Story 3 - Diagnose Stale-Focus Denials (Priority: P3)

An operator debugging a missed process-aware binding needs to know whether the denial was caused by stale metadata, missing metadata, or a true process mismatch without exposing unnecessary private process details.

**Why this priority**: Fail-closed behavior must be explainable so users can distinguish expected safety denials from configuration mistakes or compositor permission problems.

**Independent Test**: Trigger denials for stale metadata, missing metadata, permission loss, and process mismatch; verify diagnostics identify the reason, the relevant configured match rule, and the freshness threshold without leaking private command-line text.

**Acceptance Scenarios**:

1. **Given** verbose diagnostics are enabled, **When** a process-scoped binding is denied because focus metadata is stale, **Then** the diagnostic includes the denial reason, metadata age, stale threshold, and configured rule identifier.
2. **Given** verbose diagnostics are enabled, **When** focus metadata is unavailable or permission is denied, **Then** the diagnostic distinguishes that condition from a stale snapshot and from a process mismatch.
3. **Given** diagnostics are emitted, **When** process information is reported, **Then** it is limited to data the user explicitly configured or consented to inspect.

### Edge Cases

- Process-scoped bindings triggered exactly at the stale threshold use a single documented boundary rule and are tested at, below, and above that threshold.
- Global bindings without process rules remain governed by their own input and macro consent requirements and are not denied solely because focused-process metadata is stale.
- Metadata that arrives after a denied trigger does not retroactively allow the denied macro.
- Focus metadata that moves backward in time, lacks a timestamp, or cannot be ordered against the runtime clock is treated as unknown.
- Reading cached focus state from the live KDE bridge does not refresh `captured_at` or otherwise extend metadata freshness.
- Cached matching KDE focus metadata becomes stale and denies process-scoped macros if no new compositor callback arrives before the stale threshold.
- Permission revocation, unsupported compositor behavior, and missing process metadata fail closed for process-scoped bindings.
- Diagnostics avoid logging private command-line arguments, window text, or unrelated process data.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST treat focused-process metadata older than the active stale threshold as unknown for process-aware matching.
- **FR-002**: System MUST deny process-scoped macro triggers when focused-process metadata is stale, unavailable, untrusted, or missing.
- **FR-003**: System MUST prevent any macro action from being emitted for a trigger denied by stale or unavailable focus metadata.
- **FR-004**: System MUST preserve the current Lua configuration API for process-aware bindings.
- **FR-005**: System MUST keep the default focused-process stale threshold at 2 seconds unless a later specification explicitly changes it.
- **FR-006**: System MUST resume normal process-aware matching when fresh metadata that satisfies the configured rule becomes available.
- **FR-007**: System MUST distinguish stale metadata, unavailable metadata, permission denial, and process mismatch in diagnosable denial outcomes.
- **FR-008**: System MUST report stale-focus denials with enough information to identify the configured rule, freshness age, threshold, and denial reason when diagnostics are enabled.
- **FR-009**: System MUST avoid exposing process details beyond the user's configured matching scope and explicitly consented process inspection data.
- **FR-010**: System MUST keep process inspection explicit, visible in configuration, least-privilege, current-run scoped, and revocable.
- **FR-011**: System MUST fail closed with a diagnosable denial when compositor support or process metadata permission is unavailable.
- **FR-012**: System MUST include automated coverage for fresh allow, stale deny, unavailable metadata, delayed recovery, threshold boundary, and diagnostic classification behavior.
- **FR-013**: System MUST ensure active-process timestamps represent the original compositor/KWin callback receipt time used to create the focus snapshot.
- **FR-014**: System MUST NOT refresh active-process metadata freshness when cached focus state is read by the runner.
- **FR-015**: System MUST fail closed for process-scoped macros once cached KDE focus metadata exceeds the stale threshold, even when the cached process name still matches the configured rule.
- **FR-016**: System MUST include a regression test where a cached matching KDE process snapshot becomes stale without any new focus callback and denies macro execution.

### Key Entities

- **Focus Snapshot**: The most recent focused-process metadata available to process-aware matching, including freshness information and consent status.
- **Process Match Rule**: A user-configured condition that restricts a binding to a specific process identity or process class.
- **Focus Freshness Policy**: The stale threshold and boundary rule used to decide whether focus metadata is current enough to trust.
- **Stale-Focus Denial**: A denied process-scoped trigger with a reason, relevant rule reference, freshness context, and privacy-bounded diagnostic fields.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Automated tests show 100% of process-scoped trigger attempts with metadata older than the stale threshold are denied before any macro action is emitted.
- **SC-002**: Automated tests show fresh matching focused-process metadata remains eligible for process-scoped macro execution with no Lua API changes.
- **SC-003**: Threshold boundary tests cover attempts below, at, and above the 2-second default stale threshold.
- **SC-004**: Recovery tests show process-aware matching resumes on the next trigger after fresh matching metadata becomes available.
- **SC-005**: Diagnostic tests cover stale metadata, unavailable metadata, permission denial, and process mismatch as distinct denial reasons.
- **SC-006**: Privacy checks confirm stale-focus diagnostics do not include private command-line arguments, window text, or unrelated process data.
- **SC-007**: Existing process-aware Lua examples and configurations continue to load without migration.
- **SC-008**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.
- **SC-009**: Live KDE bridge regression coverage proves repeated reads of cached focus state preserve the original timestamp and do not keep matching process metadata fresh forever.

## Assumptions

- KDE Plasma Wayland remains the primary compositor target for this reliability increment.
- The focused-process metadata source may be delayed or temporarily unavailable during normal desktop operation.
- Process-aware bindings are the only bindings affected by stale focus metadata; bindings without process rules keep their existing behavior.
- The stale threshold remains a runtime policy with a 2-second default and no Lua API change in this feature.
- When KWin does not provide a separate monotonic event timestamp, the bridge's callback receipt instant is the focus snapshot timestamp; later reads must preserve that instant.
