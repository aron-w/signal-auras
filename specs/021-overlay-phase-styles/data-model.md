# Data Model: Overlay Phase Styles

## Radial Detector Phase Rule

- Represents how to recognize a radial cooldown phase from a screen sample.
- Fields remain limited to sample geometry, luminance/saturation thresholds, metric selection, progress estimate behavior, and fallback phase order.
- Must reject presentation fields: `fill`, `background`, and `opacity`.

## Progress Bar Visual Definition

- Represents a provider-neutral overlay progress bar bound to typed tracker state.
- Existing style fields remain: base `fill`, `background`, `opacity`, `label`, optional `ready`, optional `inactive`.
- New optional radial phase style fields: `activated`, `active`.
- `activated` and `active` use the same style shape as `ready` and `inactive`: optional `fill`, `background`, `opacity`, and label visibility.

## Visual Snapshot

- Represents the sanitized render update sent to a provider.
- For radial cooldown bindings, snapshot style selection order is:
  1. Base visual style.
  2. Existing compatibility fallback for activated/active phase if no Lua override exists.
  3. Matching Lua phase style override when configured.
  4. Ready or inactive style for ready/unknown/inactive states as today.
