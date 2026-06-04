# Research: Overlay Render Providers

## Native Provider First

**Decision**: Use a simple Rust-owned native overlay provider contract for v1, with an in-memory test provider and a fail-closed real-provider placeholder until compositor-specific surface creation is implemented.

**Rationale**: The first value is two translucent bars derived from already typed tracker state. A native provider keeps the trusted core in control of focus, capabilities, input pass-through, cleanup, and diagnostics without introducing a browser runtime or extra UI process.

**Alternatives considered**:
- WebView/TypeScript first: rejected for v1 because it adds another runtime and IPC/data boundary before the basic bar model is proven.
- Tauri-style app window first: rejected for v1 because normal app windows are better suited to tools and editors than game-overlay pass-through bars.
- Directly hard-code PoE2 bars in the runner: rejected because it would bypass provider-neutral declarations and make future providers harder to add.

## Provider-Neutral Overlay Model

**Decision**: Define overlay declarations in core as provider-neutral definitions: overlay id, scope, surface kind, provider id, visuals, layout/style, state bindings, diagnostics, and lifecycle state.

**Rationale**: The same user declaration should support simple native bars now and richer provider adapters later. Keeping the model independent from a specific rendering backend also keeps validation and state mapping testable without compositor hardware.

**Alternatives considered**:
- Provider-specific Lua tables: rejected because it would leak backend details into user scripts and create migration pressure.
- Free-form UI payloads: rejected because renderer providers would receive too much authority and validation would be weak.

## Typed State Snapshots Only

**Decision**: Overlay visuals bind to existing `sa.state.track(...)` tracker ids and typed emitted fields. Runtime mapping consumes `TrackerState` snapshots and emits sanitized `OverlaySnapshot` render updates.

**Rationale**: This preserves the screen-state tracking boundary: Rust owns screen capture and detector output, while overlay rendering only receives the minimum typed values needed to draw bars.

**Alternatives considered**:
- Give Lua access to tracker internals: rejected because Lua should not receive screen buffers or detector history.
- Let providers query tracker state directly: rejected because providers should be render adapters, not owners of runtime policy.

## Fail-Closed Lifecycle

**Decision**: Overlay lifecycle explicitly represents registered, active, inactive, unavailable, denied, stale, failed, and cleaned-up states. Rendering is active only while provider availability, required capabilities, trusted focus, and fresh state are all available.

**Rationale**: Desktop overlays are visible and sensitive. Users need a diagnosable closed state rather than silent fallback rendering or stale UI.

**Alternatives considered**:
- Keep the last rendered overlay visible during failure: rejected because stale overlays can mislead the user during gameplay.
- Fallback automatically to another provider: rejected because unrequested provider substitution may change security and pass-through behavior.

## Lua Registration Shape

**Decision**: Add `sa.overlay.mount(...)` as startup registration syntax parsed by the existing Lua validation crate. The parser validates duplicate ids, provider names, rectangles, opacity, styles, visual kinds, and state bindings before runtime activation.

**Rationale**: This matches the controller and state tracker pattern: Lua declares, Rust validates, and no OS-facing work happens during load.

**Alternatives considered**:
- Return overlays in the legacy `return { ... }` table: rejected because the PoE2 controller example already uses `sa.state.track(...)` style declarations.
- Dynamic runtime overlay creation from callbacks: rejected for v1 because it complicates lifecycle and capability enforcement.

## Security Boundary for Future UI Providers

**Decision**: WebView/TypeScript, Tauri-style windows, and normal tool windows are provider categories that receive sanitized overlay snapshots only. They do not own capture, focus, permissions, hotkeys, macros, or scheduling.

**Rationale**: Future UI providers should make rendering richer, not expand authority. This keeps capability decisions in the Rust core and avoids turning UI code into an automation control plane.

**Alternatives considered**:
- Let WebView own overlay logic and request host operations: rejected because it would create a second policy layer with broader attack surface.
- Exclude future providers from the model: rejected because the spec explicitly needs an adapter path for richer rendering later.
