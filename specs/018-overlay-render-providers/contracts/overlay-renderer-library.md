# Contract: Overlay Renderer Library

## Core Inputs

### OverlayDefinitionSet

Validated set of overlay definitions and their required capabilities.

Required behavior:
- Reject duplicate overlay ids.
- Aggregate active-process metadata capability for process-scoped overlays.
- Expose overlay definitions without creating any render surface.

### OverlayRuntimeInput

Runtime mapping input:
- Validated overlay definitions.
- Known state tracker definitions.
- Latest typed tracker states.
- Current scoped focus decision.
- Capability report.
- Provider availability report.

## Core Outputs

### OverlaySnapshot

Sanitized render update for one overlay.

Required behavior:
- Produce active progress-bar visual snapshots only when provider, focus, capabilities, and state freshness pass.
- Produce inactive/denied/stale/unavailable snapshots with privacy-bounded diagnostics when gates fail.
- Never include raw screen bytes, input events, compositor handles, or permission handles.

### OverlayProviderDiagnostic

Privacy-bounded diagnostic fields:
- Overlay id.
- Provider id.
- Lifecycle state.
- Reason code.
- Optional tracker id and field.
- Optional remediation string.

Must not include:
- Raw screen content.
- Window titles.
- Command-line arguments.
- Text payloads.
- Input event payloads.

## Provider Adapter

Renderer providers implement:
- `provider_id()`
- `availability()`
- `mount(snapshot)`
- `update(snapshot)`
- `hide(overlay_id, reason)`
- `cleanup(overlay_id)`

Required behavior:
- Return unavailable/denied diagnostics rather than partially activating.
- Preserve mouse and keyboard pass-through for overlay surfaces that sit above another app.
- Clean up visible surfaces on shutdown and failure.
- Never request direct macro execution, hotkey registration, screen capture, focus metadata, or permission prompts.

## V1 Native Provider

The v1 provider supports:
- Surface kind `overlay`.
- Visual kind `progress_bar`.
- Rectangles, opacity, fill/background colors, label visibility, ready style, inactive style.
- In-memory testing without compositor hardware.

The first real compositor adapter may fail closed if the current session cannot provide a safe pass-through overlay surface.

## Future Providers

The provider model reserves:
- `webview`
- `tauri_window`
- `tool_window`

These providers are render/UI adapters only. They receive sanitized snapshots and do not own automation policy.
