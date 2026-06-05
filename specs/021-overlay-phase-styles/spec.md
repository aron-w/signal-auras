# Feature Specification: Overlay Phase Styles

**Feature Branch**: `[021-overlay-phase-styles]`

**Created**: 2026-06-06

**Status**: Draft

**Input**: User description: "Improve the Lua design of `examples/poe2.lua` by clarifying separation of concerns between screen state trackers and overlay representation. Keep this on the linguistic/API side and define a clean Lua shape."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Tracker Language Stays Observational (Priority: P1)

A Lua author can read PoE2 tracker definitions and understand that trackers only describe screen observation, phase recognition, timing estimates, and typed state output.

**Why this priority**: This protects the core security and architecture boundary: screen trackers should not imply ownership of rendering, callbacks, or macro behavior.

**Independent Test**: Load a PoE2-style Lua file whose radial detector phase rules contain only recognition fields and verify it still registers the expected trackers.

**Acceptance Scenarios**:

1. **Given** a radial cooldown tracker, **When** its phase rules are inspected, **Then** the phase rules contain sampling, threshold, metric, and progress-estimation fields but no visual style fields.
2. **Given** a radial cooldown phase rule with `fill`, `background`, or `opacity`, **When** the script is validated, **Then** validation fails with a script-validation diagnostic that explains detector phase styles are not accepted.

---

### User Story 2 - Overlay Owns Phase Presentation (Priority: P2)

A Lua author can express phase-specific visual styling in the overlay declaration that binds to typed tracker state.

**Why this priority**: Rendering choices belong with provider-neutral overlay visuals, not with detector rules that read screen pixels.

**Independent Test**: Load a Lua file with an overlay progress bar that defines `ready`, `activated`, `active`, and `inactive` style overrides for a radial cooldown binding, then verify the overlay snapshots apply those styles from typed tracker phases.

**Acceptance Scenarios**:

1. **Given** a radial cooldown overlay visual bound to `remaining_ms`, **When** the tracker state is `activated`, **Then** the overlay snapshot uses the visual's `activated` style override.
2. **Given** the same visual, **When** the tracker state is `active`, **Then** the overlay snapshot uses the visual's `active` style override.
3. **Given** no phase-specific override is configured, **When** a phase is rendered, **Then** existing default overlay behavior remains compatible.

---

### User Story 3 - PoE2 Example Reads as the Intended Contract (Priority: P3)

A user opening `examples/poe2.lua` sees a clean split: trackers declare observation, overlays declare presentation, and callbacks declare automation.

**Why this priority**: The example is the public teaching artifact for this feature family and should not demonstrate misleading API boundaries.

**Independent Test**: Validate `examples/poe2.lua` through the Lua contract tests and inspect that phase color styling appears only in the overlay declaration.

**Acceptance Scenarios**:

1. **Given** `examples/poe2.lua`, **When** it is loaded by the Lua program parser, **Then** tracker and overlay registration succeeds.
2. **Given** `examples/poe2.lua`, **When** its radial detector phase definitions are reviewed, **Then** visual colors and opacity are not present under `detector.phases`.

### Edge Cases

- Overlay visuals that bind to non-radial trackers must reject radial-only phase style fields unless a future visual contract defines them.
- Existing `ready` and `inactive` overlay styles must continue to parse and behave as before.
- Invalid phase style color or opacity values must use existing overlay style validation rules.
- Denied screen, focus, provider, and stale-state behavior must remain unchanged.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST reject visual style fields inside radial detector phase rules, including `fill`, `background`, and `opacity`.
- **FR-002**: System MUST preserve detector phase fields needed for observation and typed state estimation, including sampling, luminance, saturation, metric, `progress_fill`, and `max_fill_until_ready`.
- **FR-003**: System MUST allow radial cooldown progress-bar overlay visuals to define optional `activated` and `active` style overrides using the same style validation as existing `ready` and `inactive` overrides.
- **FR-004**: System MUST apply `activated` and `active` overlay styles only when the bound tracker state is a radial cooldown in the matching phase.
- **FR-005**: System MUST preserve existing overlay behavior when the new phase style overrides are absent.
- **FR-006**: System MUST keep overlay declarations as startup registration only; this feature must not create surfaces, read pixels, consume input, synthesize input, or execute callbacks during script loading.
- **FR-007**: System MUST keep Lua isolated from raw screen buffers, input streams, compositor handles, permission handles, filesystem access, network access, and macro authority.
- **FR-008**: System MUST update `examples/poe2.lua` so detector phase rules contain recognition language only and phase-specific visual language lives in `sa.overlay.mount`.
- **FR-009**: System MUST update Lua/API contract documentation for tracker-vs-overlay ownership.
- **FR-010**: System MUST verify the change with Rust core overlay tests, Lua parser contract tests, and the PoE2 example validation path.

### Key Entities

- **Radial Detector Phase Rule**: Recognition rule for a radial cooldown phase, containing screen sample and threshold fields only.
- **Overlay Phase Style**: Optional provider-neutral style override applied by an overlay visual for a typed radial cooldown phase.
- **PoE2 Status Overlay**: Example overlay that renders tracker state as progress bars without owning screen detection.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Lua script with `fill`, `background`, or `opacity` under a radial detector phase fails validation with a detector-phase diagnostic.
- **SC-002**: A Lua script with `activated` and `active` overlay style overrides parses and produces overlay snapshots using those styles for matching radial phases.
- **SC-003**: Existing tests for `ready` and `inactive` overlay behavior continue to pass unchanged.
- **SC-004**: `examples/poe2.lua` parses successfully and contains no visual style fields under `detector.phases`.
- **SC-005**: Targeted checks pass: `cargo test -p signal-auras-core overlay`, `cargo test -p signal-auras-lua overlay`, and `cargo test --test lua_api poe2`.

## Assumptions

- This is a script API cleanup and example cleanup; no real compositor overlay capability changes are required.
- The first increment supports radial cooldown phase style overrides for progress-bar visuals only.
- Existing hardcoded radial active/activated fallback colors remain as compatibility defaults when no Lua override is present.
