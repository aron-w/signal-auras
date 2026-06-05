# Research: Overlay Phase Styles

## Decision: Reject Detector Phase Style Fields

**Rationale**: Tracker phase rules are detector configuration. Keeping `fill`, `background`, and `opacity` there implies trackers own presentation and conflicts with the observation-only state-tracker contract.

**Alternatives considered**: Continue accepting ignored style fields for compatibility. Rejected because the example is currently the main teaching artifact and silent acceptance preserves misleading language.

## Decision: Add Overlay Phase Style Overrides

**Rationale**: Existing overlay visuals already support `ready` and `inactive` style overrides. Extending the same style mechanism to radial `activated` and `active` phases gives Lua authors a coherent place for presentation without exposing raw tracker internals.

**Alternatives considered**: Hardcode all radial phase colors in Rust. Rejected because it makes the public example less expressive and leaves no provider-neutral Lua representation for phase presentation.

## Decision: Keep Fallback Colors

**Rationale**: Existing behavior should remain compatible when no phase override is configured. The default hardcoded activated/active rendering can stay as a compatibility fallback.

**Alternatives considered**: Require Lua to configure every phase style. Rejected because it would make existing overlay declarations unnecessarily verbose.
