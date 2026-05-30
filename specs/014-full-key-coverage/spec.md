# Feature Specification: Full Keyboard Key Coverage

**Feature Branch**: `014-full-key-coverage`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Create full keyboard key coverage across physical triggers and macro output, targeting all standard Linux evdev keys exposed by Keychron K5 Pro and other keyboards. Include safe key discovery diagnostics, preserve explicit current-run consent, no hidden global behavior, and existing Lua compatibility."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bind Any Exposed Keyboard Key (Priority: P1)

A user can configure any standard keyboard-like key that their keyboard exposes as a normal Linux input event and use that key as a leader, motion token, structured binding key, repeat hold token, or legacy hotkey.

**Why this priority**: Physical trigger coverage is the core value; users should not be blocked by a small hard-coded key list when their keyboard emits a standard key event.

**Independent Test**: Load scripts that use representative standard letter, number, punctuation, function, modifier, navigation, keypad, and media keys in each trigger surface and verify they normalize to the documented canonical names or compatible aliases.

**Acceptance Scenarios**:

1. **Given** a script uses a standard exposed key as `leader`, **When** the script is validated, **Then** the key is accepted and normalized without changing leader behavior.
2. **Given** a script uses standard exposed keys in `motions.trigger` and `repeat.while_held`, **When** the script is validated, **Then** each token resolves to the same canonical key vocabulary used by other trigger surfaces.
3. **Given** a script uses a standard exposed key in `bindings.trigger.key` or `hotkeys`, **When** the script is validated, **Then** the trigger is accepted unless the active provider reports that the key cannot be observed or registered.

---

### User Story 2 - Emit Supported Keys From Macros (Priority: P2)

A user can emit supported keyboard-like keys through `macro { key "..." }` using the same user-visible names accepted for physical triggers, while backends that cannot synthesize a specific key fail with a clear unsupported-output diagnostic.

**Why this priority**: Trigger coverage is incomplete if macros cannot produce common navigation, keypad, function, or media keys by the same names.

**Independent Test**: Validate macros that emit representative standard key categories and verify output planning distinguishes supported keys, unsupported backend keys, and unchanged non-key macro actions.

**Acceptance Scenarios**:

1. **Given** a macro uses `key "PageUp"`, `key "F13"`, `key "KPEnter"`, or a media key name supported by the selected output backend, **When** the macro is planned, **Then** the action uses the normalized key identity and can be emitted.
2. **Given** a macro uses a standard key name that the selected output backend cannot emit, **When** the macro is planned or executed, **Then** the system reports that the key is unsupported for output and does not silently substitute another key.
3. **Given** a macro contains text, delay, mouse, and key actions, **When** key coverage is expanded, **Then** existing non-key macro actions retain their current validation and execution behavior.

---

### User Story 3 - Discover Key Names Safely (Priority: P3)

An operator can run an explicit doctor or discovery command during the current run to press keys and see what Signal Auras would call them, which aliases remain valid, and whether each key is usable for triggers and output.

**Why this priority**: Keyboards expose model-specific keys differently; safe diagnostics let users configure keys such as Keychron K5 Pro media, navigation, or layer-adjacent keys without guessing.

**Independent Test**: Run discovery against simulated devices containing known key events, unsupported keys, denied permissions, and keys with output gaps; verify the diagnostic report is complete and no discovered state persists.

**Acceptance Scenarios**:

1. **Given** the user explicitly starts key discovery for selected devices or explicit broad discovery, **When** a standard key event is observed, **Then** diagnostics show the device path or status, raw key code, canonical token name, aliases, triggerability, emittability, and any unavailable reasons.
2. **Given** a Keychron K5 Pro Fn or layer behavior does not emit a normal input event, **When** discovery is running, **Then** the report identifies that no observable event was received rather than guessing a hidden key.
3. **Given** discovery exits, **When** the runner or later commands start, **Then** no discovered keys, device paths, permissions, aliases, or observation state are persisted from discovery.

---

### User Story 4 - Preserve Existing Lua Key Names (Priority: P4)

A user with existing scripts can keep using legacy key spellings while newer canonical names become available consistently across triggers and macro output.

**Why this priority**: Expanded coverage must not break current scripts that already use one-character keys, function keys, navigation keys, or legacy aliases.

**Independent Test**: Load existing example and contract scripts plus new alias cases; verify legacy names normalize to canonical identities and duplicate detection still catches equivalent trigger definitions.

**Acceptance Scenarios**:

1. **Given** a script uses current names such as one-character keys, `F1` through `F24`, `Left`, `Right`, `Enter`, `Tab`, `Esc`, `Escape`, `Delete`, `Del`, `Backspace`, `Space`, or `Return`, **When** the script is validated, **Then** those names remain valid aliases.
2. **Given** two bindings use different aliases for the same key, **When** duplicate trigger validation runs, **Then** the system treats them as the same physical key.
3. **Given** documentation or diagnostics mention a key, **When** aliases exist, **Then** the canonical name is shown first and legacy aliases are visible where helpful.

### Edge Cases

- Hardware-only Fn, keyboard layer, lighting, Bluetooth, pairing, or firmware controls that do not emit normal Linux input events are reported as unavailable or unobserved; the system does not infer hidden behavior.
- Keys emitted as vendor-specific, unknown, unsupported, or non-keyboard event codes are reported with raw code and unavailable reason rather than guessed as a standard key.
- Trigger support and output support may differ for the same key; diagnostics and errors must distinguish observable-but-not-emittable from emittable-but-not-observable.
- The same key may have multiple user-visible aliases; canonicalization must preserve compatibility while preventing duplicate bindings that differ only by alias.
- Permission denial, missing devices, unsupported providers, unavailable output backends, and revoked access fail closed with diagnosable errors and no macro action.
- Discovery mode does not grab, observe, or synthesize input unless the user explicitly selects the relevant current-run provider and consent path.
- Expanded key names do not grant Lua scripts filesystem, network, process, compositor, global input, or other ambient host access.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST define one canonical key vocabulary for standard Linux keyboard-like keys exposed through normal input events, covering letters, numbers, punctuation, modifiers, function keys, navigation keys, editing keys, keypad keys, system keys, and media or consumer keys commonly represented as keyboard events.
- **FR-002**: System MUST accept canonical key names consistently in `leader`, `motions.trigger`, `repeat.while_held`, `bindings.trigger.key`, legacy `hotkeys`, and `macro { key "..." }`.
- **FR-003**: System MUST preserve existing accepted aliases, including one-character keys, `F1` through `F24`, `Left`, `Right`, `Enter`, `Tab`, `Esc`, `Escape`, `Delete`, `Del`, `Backspace`, `Space`, and `Return`.
- **FR-004**: System MUST normalize aliases to a single key identity before duplicate detection, trigger matching, diagnostics, and output planning.
- **FR-005**: System MUST support all standard Linux evdev keyboard, media, navigation, editing, modifier, system, punctuation, and keypad keys that are observable as normal key events from the configured input devices.
- **FR-006**: System MUST report hardware-only or firmware-only controls that do not emit observable input events as unavailable or unobserved rather than inventing token names.
- **FR-007**: System MUST reject or report unknown, vendor-specific, non-keyboard, or unsupported key codes with raw-code diagnostics and without silently mapping them to unrelated keys.
- **FR-008**: System MUST allow macro key actions to use the same user-visible key names as triggers when the selected output backend supports emitting the key.
- **FR-009**: System MUST fail closed with a diagnosable unsupported-output result when a macro requests a known key that the selected output backend cannot emit.
- **FR-010**: System MUST provide explicit key discovery or doctor diagnostics that report only current-run device path or status, raw key code, canonical token name, aliases, triggerability, emittability, and unavailable reasons.
- **FR-011**: Discovery diagnostics MUST NOT persist discovered key state, device state, aliases, permissions, or user selections.
- **FR-012**: Discovery and runtime key observation MUST require the existing explicit current-run input consent path and MUST NOT create a daemon, autostart entry, background service, IPC endpoint, global hook, or hidden persistent listener.
- **FR-013**: Expanded key coverage MUST preserve the existing Lua configuration shape and sandbox boundaries; no broad ambient Lua capability may be added for key discovery, input observation, or output synthesis.
- **FR-014**: System MUST keep trigger-held physical modifiers independent from macro output so generated key actions emit the intended macro keys, not accidental combinations with held trigger keys.
- **FR-015**: System MUST include automated coverage for key parsing, canonicalization, alias compatibility, duplicate detection, evdev decoding, backend output mapping, unsupported-key diagnostics, discovery reports, permission denial, no persistence, and existing Lua script compatibility.
- **FR-016**: Implementation planning MUST evaluate maintained Linux input keycode/name libraries or generated upstream key tables before choosing a mapping source, and MUST justify any hand-maintained partial mapping as a temporary compatibility boundary.

### Key Entities

- **Canonical Key Token**: The stable user-visible name for one standard keyboard-like key identity.
- **Key Alias**: A backward-compatible or familiar name that resolves to a canonical key token.
- **Observed Key Event**: A current-run physical key event with device status, raw code, value state, and normalized key identity when available.
- **Trigger Support Status**: Whether a key can be used by the selected physical input or shortcut provider for leader, motion, structured binding, repeat, or hotkey activation.
- **Output Support Status**: Whether the selected macro output backend can emit a normalized key identity.
- **Discovery Diagnostic Report**: A non-persistent current-run report of observed raw codes, token names, aliases, triggerability, emittability, and unavailable reasons.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Parsing tests cover representative standard key categories across letters, numbers, punctuation, modifiers, `F1` through `F24`, navigation, editing, keypad, system, and media keys.
- **SC-002**: Lua compatibility tests show current scripts and legacy key aliases continue to validate without migration.
- **SC-003**: Trigger tests show the same canonical key vocabulary is accepted in `leader`, `motions.trigger`, `repeat.while_held`, `bindings.trigger.key`, and `hotkeys`.
- **SC-004**: Macro output tests show supported keys emit through the selected backend and unsupported backend keys produce explicit unsupported-output diagnostics with no silent substitution.
- **SC-005**: Evdev decoding tests map standard raw key codes to canonical token names and report unknown or vendor-specific codes with unavailable reasons.
- **SC-006**: Discovery tests show current-run reports include device status, raw code, canonical name, aliases, triggerability, emittability, and unavailable reasons while persisting zero discovered state after exit.
- **SC-007**: Permission-denial and unsupported-provider tests fail closed before hidden observation or macro output occurs.
- **SC-008**: Keychron K5 Pro manual or simulated coverage records observable standard keys by token and records hardware-only non-events as unavailable rather than guessed.
- **SC-009**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- Scope includes physical triggers and macro `key` output; text input, mouse actions, delay semantics, process scope matching, and repeat scheduling semantics change only as needed to share normalized key identities.
- The target key universe is standard Linux evdev keyboard-like keys, not only the visible Keychron K5 Pro layout.
- Keychron K5 Pro keys that emit normal Linux input events are in scope; Fn or firmware layer behavior that does not emit input events is outside direct automation scope and must be reported as unavailable.
- Real desktop observation and synthesis continue to rely on explicit current-run providers and permissions already required by the project.
- Exact dependency or generated-table choice belongs in implementation planning, not in this specification.
- Implementation planning will carry forward the project verification path: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible.
