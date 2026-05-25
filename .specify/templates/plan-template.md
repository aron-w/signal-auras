# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: [e.g., Rust stable toolchain from flake or NEEDS CLARIFICATION]

**Primary Dependencies**: [e.g., Wayland protocol crates, Lua runtime crate, Nix packages or NEEDS CLARIFICATION]

**Storage**: [if applicable, e.g., PostgreSQL, CoreData, files or N/A]

**Testing**: [e.g., cargo test, compositor/Wayland harness, Lua API tests or NEEDS CLARIFICATION]

**Target Platform**: [e.g., NixOS Wayland session, compositor targets or NEEDS CLARIFICATION]

**Project Type**: [e.g., Rust library with CLI/daemon/Lua bindings or NEEDS CLARIFICATION]

**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]

**Constraints**: [security/permission/reproducibility constraints, e.g., explicit consent, isolated Lua capabilities, Nix flake verification or NEEDS CLARIFICATION]

**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- Library-First: core behavior is designed as a standalone Rust library API
  before CLI, daemon, Lua, or desktop integration layers.
- Wayland/Compositor Awareness: required protocols, compositor assumptions,
  permission flows, and unsupported cases are documented.
- Rust Safety Boundaries: unsafe Rust, FFI, privileged helpers, global input,
  process inspection, and protocol adapters have explicit invariants and tests.
- Lua Extension Contract: script APIs, capability scopes, isolation boundaries,
  and breaking-change handling are specified for any scripting surface.
- NixOS Reproducibility: dependencies and verification commands are available
  through the flake or documented as explicit gaps.
- Security and Consent: sensitive automation capabilities include explicit
  user intent, scoped permissions, revocation behavior, and no ambient access.
- TDD and Testability: failing tests are planned before implementation for
  library behavior; manual compositor checks are justified with exact steps.
- Minimal Composition: new services, state stores, async runtimes, global
  registries, or abstractions are justified against simpler library composition.
- No Hidden Global Behavior: hotkeys, hooks, scripts, background processes,
  IPC, autostart, and persistence require explicit visible configuration.
- Incremental Delivery: the selected structure supports independently usable
  increments with process-aware hotkeys, macros, Lua, and later primitives kept
  as separable capabilities.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# [REMOVE IF UNUSED] Option 1: Rust workspace/library-first feature
crates/
├── signal-auras-core/      # Pure automation semantics
├── signal-auras-wayland/   # Compositor/protocol adapters
├── signal-auras-lua/       # Lua bindings and capability sandbox
└── signal-auras-cli/       # CLI/daemon entrypoints

tests/
├── contract/
├── integration/
└── compositor/

nix/
└── [packages/modules/test assets if needed]
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., background daemon] | [current need] | [why library + CLI invocation is insufficient] |
| [e.g., manual compositor verification] | [specific limitation] | [why automated Wayland harness is not practical] |
| [e.g., ambient script capability] | [specific need] | [why scoped Lua capability is insufficient] |
