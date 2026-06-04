# Data Model: Overlay Render Providers

## OverlayDefinition

Represents a startup-declared overlay.

**Fields**:
- `id`: non-empty unique overlay id.
- `scope`: scoped focus rule; defaults to explicit global only when user does not provide a scope.
- `surface_kind`: user-visible surface category, initially `overlay`.
- `provider`: selected renderer provider id.
- `visuals`: ordered `VisualDefinition` collection.
- `lifecycle`: startup lifecycle policy and cleanup requirements.

**Validation**:
- Id must be non-empty.
- Overlay ids must be unique in a program.
- Provider id must be known or rejected during validation.
- Process-scoped overlays require active-process metadata capability at runtime.

## RendererProviderId

Identifies a rendering adapter.

**Values**:
- `native`: v1 simple native progress-bar provider.
- `webview`: future WebView/TypeScript provider.
- `tauri_window`: future Tauri-style window provider.
- `tool_window`: future normal tool-window provider.

**Validation**:
- Unknown provider names fail script validation.
- Future provider ids are valid declarations but fail closed at runtime until an adapter is available.

## VisualDefinition

Represents a render element in an overlay.

**Fields**:
- `id`: non-empty unique id within the overlay.
- `kind`: initially `progress_bar`.
- `rect`: `OverlayRect`.
- `opacity`: floating-point value from 0.0 through 1.0.
- `style`: `ProgressBarStyle`.
- `label`: visibility and optional text behavior.
- `binding`: `StateBinding`.

**Validation**:
- Visual ids must be unique per overlay.
- Only `progress_bar` is supported in v1.
- Rect width and height must be positive.
- Opacity must be within 0.0 through 1.0.
- Binding must reference a known tracker id and typed field.

## OverlayRect

Represents visual placement and size.

**Fields**:
- `x`: non-negative x coordinate.
- `y`: non-negative y coordinate.
- `w`: positive width.
- `h`: positive height.

**Validation**:
- Negative coordinates are rejected.
- Zero width or height is rejected.

## ProgressBarStyle

Represents v1 progress-bar appearance.

**Fields**:
- `fill_color`: visual fill color.
- `background_color`: visual background color.
- `ready_style`: optional style applied when bound state reports ready.
- `inactive_style`: optional style applied when focus or state is inactive.

**Validation**:
- Colors must use accepted string formats documented in the Lua contract.
- Ready and inactive styles may override fill, background, opacity, or label visibility.

## StateBinding

Connects a visual to typed tracker output.

**Fields**:
- `tracker_id`: existing `StateTrackerDefinition` id.
- `field`: typed field emitted by the tracker kind.

**Valid initial bindings**:
- `heavy_stun.progress_percent` for `horizontal_progress_bar`.
- `refutation_cooldown.remaining_ms` and ready state from `radial_cooldown`.

**Validation**:
- Missing tracker id fails validation.
- Unsupported field for the tracker kind fails validation.
- Missing, stale, or inactive runtime state produces a closed overlay snapshot.

## OverlaySnapshot

Sanitized render update sent to a provider.

**Fields**:
- `overlay_id`
- `provider`
- `lifecycle_state`
- `visuals`: resolved `VisualSnapshot` entries.
- `diagnostic`: optional privacy-bounded reason when inactive or failed.

**Security rules**:
- Contains no raw screen bytes.
- Contains no input events or device handles.
- Contains no compositor handles or permission tokens.
- Contains no private window title or command-line text.

## VisualSnapshot

Resolved render-ready visual state.

**Fields**:
- `visual_id`
- `kind`
- `rect`
- `opacity`
- `fill_fraction`: 0.0 through 1.0.
- `style`
- `label`: optional sanitized label text.
- `active`: whether this visual should be rendered as active.

## OverlayLifecycleState

Represents provider/runtime state.

**States**:
- `registered`: parsed and validated, no surface created.
- `available`: provider can create a surface when runtime gates pass.
- `active`: rendered with trusted focus and fresh state.
- `inactive`: focus or source state is inactive.
- `denied`: required permission is denied or revoked.
- `stale`: required state source is stale.
- `unavailable`: provider or compositor support is unavailable.
- `failed`: provider returned an error.
- `cleaned_up`: surface cleanup completed.

**Transitions**:
- `registered` -> `available` after provider selection succeeds.
- `available` -> `active` after focus, capability, and state gates pass.
- `active` -> `inactive`/`denied`/`stale`/`unavailable`/`failed` when a gate changes.
- Any visible state -> `cleaned_up` on shutdown, provider failure, focus deactivation requiring teardown, or script activation failure.
