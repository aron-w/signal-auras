# Feature Specification: Lua Controller Runtime

**Feature Branch**: `[016-lua-controller-runtime]`

**Created**: 2026-06-02

**Status**: Implemented

**Input**: User description: "Lua as controller, Rust as library+, with focus on non-blocking efficient input / output mechanisms"

## User Scenarios & Testing

### User Story 1 - Register Lua Controllers (Priority: P1)

A user writes a main Lua file that imports other Lua files, registers hotkeys, motions, timers, and callbacks, and starts Signal Auras with that main file.

**Why this priority**: This establishes Lua as the stable controller layer while keeping Rust responsible for validated automation behavior.

**Independent Test**: Load a multi-file Lua controller that registers one process-scoped hotkey and verify startup reports validated registrations without executing any automation during load.

**Acceptance Scenarios**:

1. **Given** a valid Lua main file with local module imports, **When** Signal Auras starts, **Then** all registrations are collected, normalized, capability-checked, and reported before runtime activation.
2. **Given** duplicate or ambiguous trigger registrations, **When** Signal Auras starts, **Then** startup fails before input capture with a script-validation diagnostic.

---

### User Story 2 - Run Non-Blocking Input Callbacks (Priority: P1)

A user binds low-latency hotkeys and input motions to Lua functions without blocking the input event loop.

**Why this priority**: Input responsiveness is the core product value and must not degrade because user logic runs in Lua.

**Independent Test**: Simulate a burst of hotkey, motion, and callback events while Lua handlers perform short work; verify event dispatch remains bounded and queued work has explicit accepted, skipped, denied, or dropped disposition.

**Acceptance Scenarios**:

1. **Given** a registered Lua hotkey callback, **When** the hotkey fires, **Then** Rust validates focus/scope and schedules the Lua callback without blocking raw input polling.
2. **Given** a Lua callback is already pending for the same trigger, **When** that trigger fires again, **Then** Rust applies the bounded overload policy and records the disposition.
3. **Given** Lua callback execution exceeds the configured per-callback budget, **When** the budget is reached, **Then** subsequent input processing continues and diagnostics identify the slow callback.

---

### User Story 3 - Use Rust-Backed Output From Lua (Priority: P2)

A user calls approved Lua APIs to send keys, text, mouse actions, or later screen/pixel operations, while Rust owns the expensive and sensitive OS work.

**Why this priority**: Lua should express automation policy; Rust should own Wayland, evdev, uinput, portal, scheduler, and batching boundaries.

**Independent Test**: Register a Lua callback that sends text and keys through the Rust API; verify output is batched, ordered, capability-gated, and does not block input capture.

**Acceptance Scenarios**:

1. **Given** a Lua callback calls `sa.input.key("A")`, **When** synthesized input permission is available, **Then** Rust queues and emits the action in order.
2. **Given** synthesized input permission is denied or revoked, **When** Lua requests output, **Then** no input is emitted and the callback receives a diagnosable failure.
3. **Given** many Lua output calls occur in one callback, **When** the callback completes, **Then** Rust coalesces or batches supported output work rather than issuing one OS operation per Lua call.

---

### User Story 4 - Preserve Existing Config Scripts (Priority: P3)

A user with existing declarative Lua configuration can continue running it while the new controller API is introduced.

**Why this priority**: Lua API stability is a constitutional requirement and avoids breaking current examples.

**Independent Test**: Run existing Lua API contract tests and example scripts unchanged.

**Acceptance Scenarios**:

1. **Given** an existing declarative script, **When** Signal Auras loads it, **Then** it produces the same validated configuration as before.
2. **Given** a controller-style script, **When** Signal Auras loads it, **Then** registration APIs produce equivalent Rust-backed runtime definitions.

### Edge Cases

- Lua file imports a module outside the allowed script root.
- Lua registration tries to access denied ambient APIs such as shell, filesystem, network, debug, or unrestricted package loading.
- Lua callback throws an error after input has been captured.
- Lua callback recursively emits input that would be observed again by the input provider.
- Runtime shutdown occurs while Lua callbacks or output batches are pending.
- Screen/pixel APIs are requested without declared screen-read capability.

## Requirements

### Functional Requirements

- **FR-001**: System MUST support a controller-style Lua API for registering hotkeys, motions, press handlers, timers, and shutdown cleanup handlers during startup.
- **FR-002**: System MUST support multiple Lua files through a controlled loader rooted at the main script directory.
- **FR-003**: System MUST separate startup registration from runtime activation; no input capture, output emission, screen read, or compositor action may occur during registration.
- **FR-004**: Rust MUST normalize trigger names, reject duplicates and ambiguous overlaps, determine capability requirements, and install input/output providers after Lua registration completes.
- **FR-005**: Lua callbacks MUST run through a bounded scheduler so raw input polling, callback wakeups, repeat cancellation, shutdown, and output queue progress remain serviceable.
- **FR-006**: Rust MUST own all OS-facing operations, including global shortcut callbacks, evdev observation/grab, uinput or portal output, focus metadata, screen/pixel reads, timers, wake fds, and cleanup.
- **FR-007**: Lua output APIs MUST enqueue Rust-backed requests rather than directly blocking on OS operations.
- **FR-008**: System MUST apply deterministic overload policy for repeated triggers, slow callbacks, and full callback/output queues.
- **FR-009**: System MUST preserve explicit current-run capabilities and fail closed for denied, revoked, unavailable, or unsupported input/output/screen/process capabilities.
- **FR-010**: System MUST preserve existing declarative Lua scripts and document any new controller API as a versioned script API.
- **FR-011**: System MUST provide privacy-bounded diagnostics for registration, callback latency, queue depth, skipped/dropped work, denied capabilities, and cleanup.
- **FR-012**: System MUST define standalone Rust library contracts for registration, callback scheduling, output batching, and capability enforcement before CLI or Lua integration.
- **FR-013**: `sa.sleep` MUST yield a host timer request and MUST NOT block the runtime/event-loop thread while waiting.
- **FR-014**: Pending imperative Lua continuations MUST preserve callback overload protection until they complete, fail, or are cancelled.
- **FR-015**: Shutdown MUST cancel pending imperative Lua continuations and prevent post-cancellation output.
- **FR-016**: Controller sandbox validation MUST use the same structured Lua denied-global policy as the imperative runtime, allowing harmless local identifiers and strings while rejecting actual ambient API access.
- **FR-017**: Declarative Lua compatibility loading MUST preserve its existing source-parser behavior, but denied ambient API tokens MUST be owned by the shared sandbox policy rather than duplicated ad hoc constants.

### Key Entities

- **Lua Controller Script**: Main Lua file plus allowed local modules that register automation behavior.
- **Controller Registration**: A normalized trigger, scope, mode, callback reference, required capabilities, and overload policy.
- **Lua Callback Task**: Runtime invocation of a registered Lua function with trigger context, budget, status, and diagnostics.
- **Rust Operation Request**: Safe queued request for input output, screen read, focus query, or timer behavior.
- **Runtime Capability Grant**: Current-run permission state for sensitive automation capabilities.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Valid multi-file Lua controller scripts load, register, and activate without hidden OS effects during registration.
- **SC-002**: Simulated mixed input dispatch maintains existing runtime targets: p95 <= 20 ms and p99 <= 50 ms before Lua callback execution.
- **SC-003**: A stress test with at least 10,000 trigger opportunities keeps per-trigger pending work bounded and records every accepted, skipped, denied, dropped, cancelled, or failed disposition.
- **SC-004**: Slow or failing Lua callbacks do not block shutdown, repeat cancellation, callback wakeups, or raw input pass-through.
- **SC-005**: Denied capabilities execute no OS action and produce diagnosable user-facing errors.
- **SC-006**: Existing Lua compatibility tests and examples continue to pass unchanged.
- **SC-007**: Verification passes with documented Nix commands for formatting, linting, tests, and flake checks where feasible.
- **SC-008**: Contract tests show `sa.sleep` creates pending work, resumes only on a timer wake, and is cancellable with no host-thread sleep call.
- **SC-009**: Controller sandbox tests show ambient globals are denied through `mlua` execution, while denied API names inside strings or local variable names do not fail validation.

## Assumptions

- V1 uses a two-phase model: Lua registers behavior at startup; dynamic runtime registration is out of scope except enable/disable of already registered handles.
- V1 embeds real Lua 5.4-compatible execution, but host APIs remain capability-scoped and sandboxed.
- Lua callbacks are intended for short policy decisions; expensive screen, input, process, and compositor operations are Rust-backed queued APIs.
- Pure Lua code that spins without yielding still requires a later runtime budget/preemption follow-up; this increment schedules host-yielding callbacks such as `sa.sleep`.
- Rust remains the trusted core and owns all safety, lifetime, event-loop, and OS permission boundaries.
- KDE Plasma Wayland on NixOS remains the primary real-desktop target; unsupported compositors fail explicitly.
- Pixel/image checks may be exposed through this controller model later, but this feature focuses first on non-blocking input/output and callback scheduling.
