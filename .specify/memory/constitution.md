<!--
Sync Impact Report
Version change: 1.0.0 -> 2.0.0
Modified principles:
- Spec-Led Delivery -> Library-First Automation Core
- Independently Deliverable User Stories -> Wayland-First and Compositor-Aware
- Testable Quality Gates -> Rust Core with Explicit Safety Boundaries
- Minimal, Explicit Architecture -> Lua as the Stable Extension Layer
- Traceable Operations and Review -> NixOS-First Reproducibility
Added principles:
- Security and Consent by Design
- Testable Automation Behavior
- Minimal, Composable Architecture
- No Hidden Global Behavior
- Incremental Feature Delivery
Added sections:
- Project Goal
- Non-Negotiable Constraints
Removed sections: none
Templates requiring updates:
- updated: .specify/templates/plan-template.md
- updated: .specify/templates/spec-template.md
- updated: .specify/templates/tasks-template.md
- not applicable: .specify/templates/commands/*.md (directory absent)
- reviewed: AGENTS.md
Follow-up TODOs: none
-->
# Signal Auras Constitution

## Project Goal

Signal Auras is an AHK-like automation tool for Wayland on NixOS. It MUST
support process-aware hotkey rebinding, macro execution, Lua scripting, and
later desktop automation primitives such as pixel checks, image checks, window
queries, and other compositor-aware actions.

## Core Principles

### I. Library-First Automation Core
Every feature MUST be implemented as a standalone Rust library API before it is
exposed through a CLI, daemon, Lua binding, or desktop integration. Libraries
MUST have explicit inputs, outputs, errors, and tests. Integration layers MAY
compose libraries, but MUST NOT contain core automation behavior that cannot be
tested without the integration layer.

Rationale: Library-first design keeps automation primitives reusable,
independently testable, and safe to expose through multiple user-facing layers.

### II. Wayland-First and Compositor-Aware
All desktop automation behavior MUST target Wayland semantics first and MUST
document compositor-specific assumptions, protocols, permissions, and
limitations. Features that depend on compositor extensions, portals, input
methods, virtual keyboard protocols, screencopy protocols, or process/window
metadata MUST fail explicitly when unavailable. X11 compatibility MUST NOT
drive architecture unless a future spec defines it as a separate adapter.

Rationale: Wayland security and compositor diversity are central constraints,
not portability details that can be patched in later.

### III. Rust Core with Explicit Safety Boundaries
The trusted core MUST be written in Rust. Unsafe Rust, FFI, privileged helpers,
global input capture, process inspection, and compositor protocol bindings MUST
be isolated behind small modules with documented invariants and tests. Any
feature crossing a safety boundary MUST define ownership, lifetime, threading,
error, and permission behavior in the plan before implementation.

Rationale: Automation tools interact with sensitive desktop surfaces; safety
boundaries must be deliberate and reviewable.

### IV. Lua as the Stable Extension Layer
Lua scripting is the stable user-facing extension layer. Script APIs MUST be
versioned, documented, deterministic where practical, and backed by Rust library
contracts. Scripts MUST run with explicit capabilities and isolation boundaries;
they MUST NOT receive ambient access to global input, process data, filesystem
paths, network access, or compositor actions without declared consent and
permission checks.

Rationale: Users need a durable scripting surface, while the host must retain
control over sensitive automation capabilities.

### V. NixOS-First Reproducibility
Development, testing, packaging, and runtime examples MUST be reproducible on
NixOS through the project flake or documented Nix commands. New dependencies,
native libraries, compositor test tools, Lua packages, and system capabilities
MUST be represented in Nix configuration or explicitly documented as unavailable
to Nix builds. Plans MUST include the Nix commands used to verify the feature.

Rationale: NixOS reproducibility is a primary product constraint and the basis
for reliable desktop automation testing.

### VI. Security and Consent by Design
Security, permissions, reproducibility, and script isolation are
NON-NEGOTIABLE. Features that observe input, synthesize input, inspect
processes, read screen contents, execute macros, or run scripts MUST require
explicit user intent, scoped permissions, visible configuration, and revocation
paths. Silent privilege escalation, hidden persistence, broad ambient access,
and unclear consent flows are prohibited.

Rationale: A desktop automation tool can affect private data and user control;
trust must be designed into every capability.

### VII. Testable Automation Behavior
TDD is mandatory. Every library behavior MUST start with failing tests before
implementation. Plans MUST define automated tests for parsing, matching,
rebinding, macro scheduling, script capability enforcement, error handling, and
compositor protocol adapters when a harness can exercise them. Manual
verification is allowed only for compositor interactions that cannot yet be
automated, and the plan MUST explain the limitation and exact procedure.

Rationale: Hotkeys and macros are stateful and timing-sensitive; regressions
must be caught before users bind their workflow to the tool.

### VIII. Minimal, Composable Architecture
The architecture MUST remain small, functional where practical, and composed of
pure parsing/matching/planning logic plus narrow side-effect adapters. New
daemons, background services, state stores, plugin systems, async runtimes, or
global registries MUST be justified against a simpler library composition.
Functional patterns are preferred for transformations, matching, configuration
evaluation, and macro planning.

Rationale: Composable primitives make automation behavior easier to test,
reason about, and expose safely to Lua.

### IX. No Hidden Global Behavior
No feature MAY install global hotkeys, hooks, scripts, background processes,
autostart entries, IPC endpoints, or persistent state without explicit
configuration and user-visible documentation. Defaults MUST be inert or
least-privilege. Process-aware behavior MUST be scoped to declared match rules
and MUST expose how a decision was made.

Rationale: Users must be able to predict, audit, and disable every automation
effect that can influence their desktop.

### X. Incremental Feature Delivery
Features MUST be delivered in independently usable increments, starting with
the smallest library-backed behavior that proves the user value. Process-aware
hotkey rebinding, macro execution, Lua scripting, pixel/image checks, and later
automation primitives MUST be planned as separable capabilities with explicit
contracts and tests before integration.

Rationale: Incremental delivery keeps a sensitive automation stack reviewable
and avoids coupling future primitives to premature design choices.

## Non-Negotiable Constraints

Security, permissions, Nix reproducibility, and Lua script isolation MUST be
treated as release blockers for every feature. A spec that touches input,
screen contents, process metadata, macro execution, scripting, IPC, persistence,
or compositor protocols MUST include security and consent requirements. A plan
for such a feature MUST include permission boundaries, revocation behavior,
NixOS verification commands, and script/API isolation checks.

The Rust core MUST own automation semantics. Lua bindings, CLIs, daemons, and
desktop integrations MUST call into tested libraries rather than duplicating
behavior. Public script APIs MUST be stable by default; breaking script API
changes require migration notes and a MAJOR version bump for the script API,
even when the project constitution version is unchanged.

Wayland limitations MUST be represented as explicit behavior. Unsupported
compositors, missing protocols, denied permissions, and unavailable portals MUST
produce diagnosable errors instead of degraded hidden behavior.

## Development Workflow

The required sequence for feature work is: specify, clarify when needed, plan,
generate tasks, implement with tests first, verify, and then review or commit.
Each step MUST consume the latest artifact from the previous step and MUST
preserve the constitution gates.

Before implementation starts, the Constitution Check in the plan MUST pass or
must list justified violations in Complexity Tracking. After design artifacts
are produced, the Constitution Check MUST be re-evaluated against the selected
library API, Rust safety boundaries, Lua contracts, Nix verification path,
security model, and compositor behavior.

Tasks MUST be grouped by independently deliverable user story, include exact
file paths, identify safe parallel work, and include test-first verification
work for each story. A story is not complete until its tests pass or an
approved manual compositor verification procedure has been run and recorded.

## Governance

This constitution supersedes conflicting project practices for feature
specification, planning, task generation, implementation, review, and release
readiness. Future specs, plans, and tasks MUST explicitly satisfy the current
principles; missing security, permission, reproducibility, or script isolation
coverage blocks planning and implementation.

Amendments require a documented change to this file, a Sync Impact Report that
lists affected templates and guidance, and review of all dependent Spec Kit
templates. Changes that remove or redefine a principle require a MAJOR version
bump. Changes that add a principle or materially expand governance require a
MINOR version bump. Clarifications, wording improvements, and non-semantic
template alignment require a PATCH version bump.

Compliance review is required during specification, plan creation, task
generation, and implementation completion. Reviewers MUST verify that artifacts
trace to this constitution, that TDD evidence exists for library behavior, that
Nix verification commands are documented, and that sensitive automation
capabilities have explicit consent and isolation boundaries.

**Version**: 2.0.0 | **Ratified**: 2026-05-25 | **Last Amended**: 2026-05-25
