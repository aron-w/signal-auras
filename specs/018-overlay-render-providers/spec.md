# Feature Specification: Overlay Render Providers

**Feature Branch**: `[018-overlay-render-providers]`

**Created**: 2026-06-04

**Status**: Implemented

**Input**: User description: "Create a provider-based on-screen rendering system for scoped overlays. V1 should use the simplest reliable provider to draw WeakAuras-like translucent progress bars for PoE2 tracker states, while modeling WebView/TypeScript, Tauri-style windows, and normal tool windows as future renderer adapters. Lua declares overlays with `sa.overlay.mount(...)`, binds visuals to existing `sa.state.track(...)` ids and typed fields, and preserves security boundaries."

## User Scenarios & Testing

### User Story 1 - Show PoE2 Tracker Bars (Priority: P1)

A user running the PoE2 tracker example can see translucent Heavy Stun and Refutation status bars above the game only while the trusted PoE2 focus is active.

**Why this priority**: This is the first visible value of the state-tracking work and proves scoped overlays without adding automation reactions.

**Independent Test**: Load a configuration that declares the two PoE2 state trackers and two overlay progress bars; simulate trusted focus and typed tracker state snapshots, then verify visible bar state updates, inactive focus hides or disables rendering, and no input is consumed.

**Acceptance Scenarios**:

1. **Given** trusted PoE2 focus and Heavy Stun state with `progress_percent`, **When** the overlay is active, **Then** the Heavy Stun bar renders at the configured position with the configured size, opacity, fill color, background color, and label visibility.
2. **Given** trusted PoE2 focus and Refutation cooldown state with `remaining_ms`, **When** Refutation is cooling down, **Then** the Refutation bar renders as cooling down with progress derived from the typed cooldown state.
3. **Given** trusted PoE2 focus and Refutation is ready, **When** the overlay receives the ready state, **Then** the Refutation bar uses the configured ready or inactive style rather than showing a stale cooldown.
4. **Given** focus is inactive, stale, unavailable, denied, ambiguous, or non-matching, **When** tracker states change, **Then** scoped overlays are hidden or inactive and no screen overlay remains trusted as active.

---

### User Story 2 - Declare Provider-Neutral Overlays (Priority: P2)

A user can declare an overlay once with a provider selection, surface kind, layout, style, visuals, state bindings, and lifecycle rules without coupling the overlay model to one rendering backend.

**Why this priority**: The first bars need a small model that can grow into richer UI providers without changing the security model or overlay declarations.

**Independent Test**: Parse valid and invalid overlay declarations, including provider selection, duplicate visual ids, invalid rectangles, invalid opacity, unsupported providers, and missing bindings.

**Acceptance Scenarios**:

1. **Given** a valid `sa.overlay.mount(...)` declaration with a supported provider, **When** the script loads, **Then** the overlay definition validates before runtime activation and records provider, scope, surface kind, visuals, layout, style, state bindings, and lifecycle.
2. **Given** an overlay declaration selects an unavailable provider, **When** the runtime activates overlays, **Then** the overlay fails closed with a diagnostic and no untrusted fallback rendering is created.
3. **Given** a visual references a missing tracker id or a typed field that is not emitted by that tracker, **When** the overlay definition validates, **Then** validation fails before runtime activation.

---

### User Story 3 - Keep Overlay Rendering Isolated (Priority: P3)

A user can enable overlays without granting Lua or future UI providers raw screen data, input devices, compositor handles, filesystem access, network access, or authority over hotkeys, macros, capture, or capability decisions.

**Why this priority**: Rendering is sensitive because it sits above the desktop, but it must not become a backdoor around the trusted Rust-owned automation boundaries.

**Independent Test**: Attempt to use overlay declarations and future-provider placeholders to access forbidden capabilities; verify only typed state snapshots and approved render commands cross the overlay boundary.

**Acceptance Scenarios**:

1. **Given** a Lua overlay declaration, **When** the script loads and runtime starts, **Then** Lua can declare visuals and state bindings but cannot read raw screen buffers, input streams, compositor objects, or ambient filesystem/network resources.
2. **Given** a future WebView/TypeScript renderer provider is modeled, **When** it receives data, **Then** it receives only sanitized typed state and render model data, not ownership of hotkeys, macros, screen capture, permissions, or process focus decisions.
3. **Given** overlay rendering is unavailable, permission is denied, focus is inactive, or state is missing or stale, **When** the user runs the example, **Then** diagnostics explain the closed state and no overlay consumes input or triggers macros.

### Edge Cases

- Two visuals use the same id within one overlay.
- Overlay rectangle has negative coordinates, zero width or height, or dimensions outside the allowed bounds.
- Opacity is outside the inclusive 0.0 to 1.0 range.
- Fill, background, label, ready, or inactive style fields are malformed or incomplete.
- A selected provider is unknown, unsupported on the current compositor, denied by permissions, or cannot create a pass-through surface.
- A required state tracker is not declared, has not produced a sample, is stale, or emits a different field type than the visual expects.
- Multiple overlays or visuals bind to the same tracker state and must receive consistent snapshots.
- Overlay shutdown, script reload failure, or runtime error occurs while a surface is visible.
- Mouse movement, clicks, keyboard input, hotkeys, motions, and macros continue through the game normally while overlays are visible.
- Existing local edits in `examples/poe2.lua` must not be overwritten by specification, planning, or later implementation work.

## Requirements

### Functional Requirements

- **FR-001**: System MUST support a provider-neutral overlay model with overlay id, scope, surface kind, selected renderer provider, visuals, layout, style, state bindings, diagnostics, and lifecycle.
- **FR-002**: System MUST support Lua declaration of overlays through `sa.overlay.mount(...)` during startup registration without creating render surfaces, reading the screen, consuming input, or executing macros during registration.
- **FR-003**: System MUST allow the renderer provider to be selected declaratively and MUST default the v1 PoE2 examples to the simplest supported provider for translucent progress bars.
- **FR-004**: System MUST define a v1 native overlay provider that can render progress-bar visuals from typed state snapshots and keep mouse input pass-through.
- **FR-005**: System MUST model WebView/TypeScript, Tauri-style application windows, and normal tool windows as future renderer provider adapters without requiring them for the first PoE2 bars.
- **FR-006**: System MUST treat WebView/TypeScript providers as rendering and UI providers only; they MUST NOT own hotkeys, macros, screen capture, capability decisions, process focus decisions, or automation scheduling.
- **FR-007**: System MUST support at least a `progress_bar` visual with position, size, opacity, fill color, background color, optional label visibility, and inactive or ready style.
- **FR-008**: System MUST bind overlay visuals only to existing typed state tracker ids and typed emitted fields, including Heavy Stun `progress_percent` and Refutation cooldown `remaining_ms`.
- **FR-009**: System MUST render the Heavy Stun progress bar from `heavy_stun.progress_percent` when the associated state source is fresh and scoped focus is trusted.
- **FR-010**: System MUST render the Refutation cooldown bar from `refutation_cooldown.remaining_ms` while cooling down and switch to the configured ready style when the state source reports ready.
- **FR-011**: System MUST render scoped overlays only while the target focus is trusted and required current-run capabilities are available.
- **FR-012**: System MUST fail closed with diagnostics when a provider is unavailable, permission is denied, focus is inactive or untrusted, a state source is missing, a source is stale, or a binding references an invalid field.
- **FR-013**: System MUST validate overlay declarations for duplicate visual ids, invalid provider names, invalid rectangles, invalid opacity, malformed styles, unsupported visual kinds, and missing state bindings.
- **FR-014**: System MUST preserve Lua and future UI provider security boundaries; scripts and UI providers MUST NOT receive raw screen buffers, input devices, compositor handles, ambient filesystem access, ambient network access, or permission handles.
- **FR-015**: System MUST ensure overlay updates do not trigger macros, synthesize input, capture input, consume input, or alter existing hotkey and motion scheduling.
- **FR-016**: System MUST clean up overlay surfaces on shutdown, provider failure, focus deactivation, or script activation failure so stale overlays are not left on screen.
- **FR-017**: System MUST provide privacy-bounded diagnostics for overlay id, provider, lifecycle state, focus/capability denial reason, stale or missing state source, and validation errors without logging raw screen content or private window text.
- **FR-018**: System MUST preserve existing user edits in `examples/poe2.lua` when later adding overlay examples or migrations.

### Key Entities

- **Overlay Definition**: A startup-declared overlay with id, scope, surface kind, selected provider, lifecycle policy, diagnostics identity, and a collection of visuals.
- **Renderer Provider**: A named adapter capable of turning sanitized overlay snapshots into visible UI surfaces, with availability, permission, pass-through, cleanup, and diagnostics behavior.
- **Overlay Surface**: The runtime on-screen presentation created by a provider for a scoped overlay while focus and capabilities are trusted.
- **Visual**: A declared render element such as a progress bar with layout, style, opacity, label behavior, and state binding.
- **State Binding**: A reference from a visual to a tracker id and typed emitted field from the screen-state tracking system.
- **Overlay Snapshot**: A sanitized, Rust-owned render update containing visual geometry, style, lifecycle state, and typed state values, excluding raw screen or input data.
- **Lifecycle State**: The overlay state such as registered, available, active, inactive, denied, stale, unavailable, failed, or cleaned up.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Valid Lua declarations for two PoE2 overlay bars load and validate without modifying existing tracker declarations or registering any macro reaction.
- **SC-002**: Parser and validation tests reject duplicate visual ids, invalid providers, invalid rectangles, invalid opacity, malformed styles, unsupported visual kinds, and missing or mistyped state bindings.
- **SC-003**: State-to-visual tests cover Heavy Stun progress, Refutation cooling down, Refutation ready, inactive focus, stale source, and missing source.
- **SC-004**: Provider selection tests verify supported-provider activation and unavailable-provider diagnostics without requiring real KDE overlay hardware.
- **SC-005**: Overlay update tests verify visible updates do not synthesize input, consume input, trigger callbacks, trigger macros, or expose raw screen data.
- **SC-006**: Manual KDE/PoE2 verification confirms two translucent bars render above the game, update from existing trackers, clean up on shutdown, and allow mouse input to pass through.
- **SC-007**: Denied permissions, unsupported providers, inactive focus, stale state, and missing state produce diagnosable closed states with no active overlay surface.
- **SC-008**: Feature verification passes with documented Nix commands for formatting, linting, tests, and flake checks where feasible.

## Assumptions

- Lua remains the stable automation and declaration layer for this feature.
- The first deliverable prioritizes a simple native overlay renderer over a WebView renderer to reduce runtime and permission risk.
- WebView/TypeScript and Tauri-style support are future provider adapters for richer overlays, normal tool windows, editors, or configuration UI.
- Existing PoE2 state trackers from `017-poe2-state-tracking` are the source of truth for v1 bar values.
- The initial overlay surface is scoped to KDE Plasma Wayland on NixOS and fails closed on unsupported compositors or denied permissions.
- V1 overlay visuals are progress bars only; arbitrary shapes, animations, rich text, drag editing, persistence, and interactive overlay controls are future work.
