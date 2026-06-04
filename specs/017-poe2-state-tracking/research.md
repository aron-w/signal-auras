# Research: PoE2 Screen State Tracking

## Decision: Detector Schemas Are Rust-Owned

**Rationale**: The requirement forbids user-declared emitted fields, and Rust must own automation semantics. Each detector kind defines its config validation and typed state.

**Alternatives considered**: A Lua-declared schema was rejected because it would let scripts invent state shapes and weaken diagnostics.

## Decision: Use Deterministic Fixture Byte Inputs for This Increment

**Rationale**: The workspace has no image/video decoding dependency, and adding one would widen Nix/native scope. For this increment, tests exercise detector progression and state estimation with deterministic bytes from the committed `.webm` fixtures plus explicit labeled expectations.

**Alternatives considered**: Adding ffmpeg/GStreamer or a video crate was rejected for v1 because it adds native dependencies and is not required to validate tracker semantics, gating, or Lua registration.

## Decision: Screen Read Is a New Capability Kind

**Rationale**: Screen contents are sensitive. A dedicated `screen_read` capability lets registration require explicit consent and lets runtime fail closed before capture.

**Alternatives considered**: Reusing active window metadata or synthesized input capabilities was rejected because screen content has a different privacy boundary.

## Decision: Polling Shares One Screen Sample

**Rationale**: Multiple trackers at the same cadence should not multiply capture sessions. A Rust poller batches due trackers, acquires at most one sample, then classifies each ROI.

**Alternatives considered**: Per-tracker capture was rejected for performance and privacy because it increases capture surfaces and duplicated work.
