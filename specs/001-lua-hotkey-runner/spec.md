# Feature Specification: Lua Hotkey Runner

**Feature Branch**: `001-lua-hotkey-runner`

**Created**: 2026-05-25

**Status**: Draft

**Input**: User description: "Specify v1 Wayland automation CLI runner that loads one sandboxed Lua file, registers scoped hotkey macros, prompts for missing scope, runs until Ctrl-C, and logs runtime stats"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a scoped Lua hotkey macro (Priority: P1)

A user starts the runner from a terminal with exactly one Lua automation file. The file declares the process scope and a hotkey macro. The runner validates the file, reports the effective startup configuration, registers the hotkey only for the declared process scope, and remains active until stopped.

**Why this priority**: This is the smallest useful v1 flow: a user can bind a process-aware macro without any hidden global behavior.

**Independent Test**: Can be tested by running the CLI with a Lua file that declares a process-scoped `F5` macro and verifying that startup logs show the script, scope, hotkey, and registration result before the runner waits for input.

**Acceptance Scenarios**:

1. **Given** a valid Lua file declaring `scope = { processes = { "poe2.exe" } }` and an `F5` macro, **When** the user starts the runner with that file, **Then** the runner validates the script and registers the `F5` macro for `poe2.exe` only.
2. **Given** a valid scoped Lua file, **When** the configured compositor protocol or permission needed for hotkey registration is unavailable, **Then** the runner exits before registration and logs a diagnosable error.

---

### User Story 2 - Choose scope when Lua omits it (Priority: P2)

A user starts the runner with a valid Lua file that defines hotkeys but omits scope. The runner asks in the terminal whether to use one or more process names or explicit global scope for the current run.

**Why this priority**: Users can reuse simple scripts while still making an explicit consent decision before any hotkey becomes active.

**Independent Test**: Can be tested by running the CLI with a scope-free Lua file, selecting process names or global scope in the terminal prompt, and verifying that the selected scope is logged and used only for that run.

**Acceptance Scenarios**:

1. **Given** a Lua file with hotkeys and no scope, **When** the user selects process names from the terminal prompt, **Then** the runner registers the hotkeys only for those process names for the current run.
2. **Given** a Lua file with hotkeys and no scope, **When** the user selects global scope in the terminal prompt, **Then** the runner registers global hotkeys only after logging the explicit global selection.
3. **Given** a Lua file with hotkeys and no scope, **When** the user cancels the prompt, **Then** the runner exits without registering any hotkeys.

---

### User Story 3 - Trigger macros only for matching active process (Priority: P3)

A user presses a registered hotkey while the runner is active. The runner compares the active process name with the configured scope and executes the macro only when the match succeeds.

**Why this priority**: Process-aware gating is the primary safety and utility distinction from an unscoped global hotkey tool.

**Independent Test**: Can be tested by simulating or observing active process names and verifying that `F5` sends `/hideout` only when the active process is `poe2.exe`.

**Acceptance Scenarios**:

1. **Given** `F5` is configured for `poe2.exe`, **When** `poe2.exe` is active and the user presses `F5`, **Then** the runner executes the macro actions in order: `Enter`, text `/hideout`, `Enter`.
2. **Given** `F5` is configured for `poe2.exe`, **When** another process is active and the user presses `F5`, **Then** the runner denies the macro execution, performs no macro action, increments denied-action stats, and logs the scope mismatch.
3. **Given** a macro contains key presses, text, and delays, **When** the macro is allowed to run, **Then** each action is attempted in the declared order and failures stop the current macro with a logged reason.

---

### User Story 4 - Stop cleanly and see final stats (Priority: P4)

A user stops the terminal runner with Ctrl-C. The runner unregisters hotkeys, stops accepting triggers, and prints a final summary of useful runtime stats.

**Why this priority**: The first release must be predictable and auditable during shutdown, especially because it controls desktop input.

**Independent Test**: Can be tested by starting the runner, triggering allowed and denied macros, pressing Ctrl-C, and verifying that shutdown logs include final counts and no hotkeys remain active.

**Acceptance Scenarios**:

1. **Given** the runner is active, **When** the user presses Ctrl-C, **Then** the runner unregisters hotkeys and exits cleanly.
2. **Given** macros have been triggered during the run, **When** the runner exits, **Then** the final summary includes trigger counts, macro success counts, macro failure counts, denied-action counts, and elapsed runtime.

### Edge Cases

- The command receives zero Lua file arguments or more than one Lua file argument.
- The Lua file path cannot be read, is not valid Lua, or does not return the expected configuration shape.
- The Lua file defines no hotkeys, duplicate hotkeys, unsupported hotkey names, unsupported macro actions, malformed delay values, or invalid process names.
- The Lua file attempts filesystem, network, process, compositor, global input, or other ambient access outside the allowed script API.
- The Lua file omits scope and the terminal is not interactive.
- The user selects global scope by mistake and cancels before confirming.
- The active process cannot be determined when a hotkey fires.
- A compositor protocol, portal, virtual keyboard capability, input method, or permission required for registration or macro execution is missing or denied.
- A macro is triggered again while the same macro is still executing.
- Ctrl-C occurs while a macro is running or while the scope prompt is waiting for input.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST expose a terminal-started CLI runner that accepts exactly one Lua file argument and rejects zero or multiple file arguments before loading any script.
- **FR-002**: The system MUST run on a NixOS Wayland session and MUST report diagnosable errors for unsupported compositors, missing protocols, unavailable portals, denied permissions, or unavailable process metadata before relying on those capabilities.
- **FR-003**: The system MUST load and validate the Lua script shape before registering any hotkeys.
- **FR-004**: The Lua script MUST return a configuration object with `hotkeys` and MAY include `scope`.
- **FR-005**: The system MUST support this v1 Lua configuration shape:

  ```lua
  return {
    scope = { processes = { "poe2.exe" } },
    hotkeys = {
      ["F5"] = macro {
        key "Enter",
        text "/hideout",
        key "Enter",
      },
    },
  }
  ```

- **FR-006**: The system MUST allow macro actions for key presses, text input, and delays, and MUST execute allowed macro actions in the order declared by the script.
- **FR-007**: The system MUST reject unsupported macro actions, malformed macro action parameters, empty macros, duplicate hotkey definitions, and unsupported hotkey identifiers before registering hotkeys.
- **FR-008**: The system MUST support process-scoped hotkeys when the Lua script declares `scope.processes` with one or more user-visible executable or process names.
- **FR-009**: When a Lua script omits scope, the system MUST prompt in the terminal for either one or more process names or explicit global scope before registering hotkeys.
- **FR-010**: The system MUST treat scope selected from the terminal prompt as current-run state only and MUST NOT persist it unless a future feature explicitly adds persistent configuration.
- **FR-011**: The system MUST require explicit terminal selection before registering any global hotkey behavior.
- **FR-012**: The system MUST NOT register global hotkeys by default or infer global scope from an absent, invalid, or empty scope.
- **FR-013**: The system MUST compare each hotkey trigger with the configured active-process scope before executing any macro action.
- **FR-014**: The system MUST deny scoped macro execution when the active process is unknown, unavailable, or not included in the configured process list.
- **FR-015**: The system MUST run until the user stops it with Ctrl-C or until it encounters an unrecoverable startup/runtime error that requires shutdown.
- **FR-016**: On Ctrl-C, the system MUST unregister hotkeys, stop accepting new triggers, and print a final shutdown summary before exiting.
- **FR-017**: The system MUST log startup configuration, effective scope, script validation result, hotkey registration results, trigger counts, successful macro counts, failed macro counts, denied-action counts, permission failures, and shutdown summary.
- **FR-018**: The system MUST sandbox Lua scripts so scripts have no ambient filesystem, network, process, compositor, global input, shell, environment, or host state access.
- **FR-019**: The system MUST expose only the approved Lua macro-building API and declared configuration data needed for this feature.
- **FR-020**: The system MUST make all consent decisions visible in terminal output, including process scope, explicit global selection, denied permissions, and denied scope mismatches.
- **FR-021**: The system MUST define the standalone Rust library behavior for script validation, scope matching, macro planning, stats collection, and permission/error modeling before CLI, Lua binding, or Wayland adapter behavior is implemented.
- **FR-022**: The system MUST keep compositor-specific input capture, synthesized input, active-process discovery, protocol bindings, and permission checks behind explicit safety boundaries with diagnosable errors.
- **FR-023**: The system MUST provide automated verification for script validation, scope matching, macro action ordering, stats accounting, consent flow decisions, and Lua isolation; compositor interactions that cannot be automated MAY use documented manual verification in the implementation plan.

### Key Entities *(include if feature involves data)*

- **Runner Invocation**: The terminal command state for one run, including the single Lua file path, whether stdin is interactive, startup result, and shutdown reason.
- **Lua Automation Configuration**: The validated configuration returned by the Lua file, including optional scope and required hotkey definitions.
- **Scope Selection**: The effective run scope, either declared process names from the script, process names selected in the terminal prompt, or explicitly selected global scope.
- **Hotkey Binding**: A registered hotkey plus its effective scope and macro definition.
- **Macro**: An ordered list of supported actions: key press, text input, and delay.
- **Runtime Stats**: Per-run counters and timing data for triggers, successful macros, failed macros, denied actions, permission failures, registration results, and shutdown summary.
- **Diagnosable Error**: A user-visible error that identifies the failed capability, operation phase, and likely remediation path without silently degrading behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can start the runner with a valid process-scoped Lua file and see registered hotkeys plus effective scope in terminal output before the runner waits for triggers.
- **SC-002**: In acceptance testing, the sample `F5` macro sends `/hideout` only when the active process matches `poe2.exe` and performs no macro action for non-matching processes.
- **SC-003**: For scope-free Lua files, 100% of runs require an interactive terminal selection before registration, and global scope is registered only after explicit global selection.
- **SC-004**: Invalid Lua, invalid configuration shape, denied permissions, and unsupported compositor capability cases all stop before hotkey registration and produce a diagnosable error.
- **SC-005**: Ctrl-C shutdown completes without leaving registered hotkeys active and prints final runtime stats for the completed run.
- **SC-006**: Automated verification covers script validation, scope decisions, action ordering, stats accounting, and Lua isolation, with any non-automated compositor checks documented as exact manual steps.
- **SC-007**: Feature verification is reproducible through documented Nix commands during planning and implementation.

## Assumptions

- v1 targets a terminal-started runner on a NixOS Wayland session; graphical prompts, daemons, autostart, IPC, persistent state, and X11 compatibility are out of scope.
- Process matching is based on user-visible executable or process names; exact discovery mechanics and compositor-specific metadata sources are deferred to planning.
- Global scope is allowed in v1 only through explicit terminal selection for the current run.
- Lua is the stable user-facing extension layer, while the trusted automation semantics are owned by Rust library APIs per the project constitution.
- The first implementation may use mocked or documented compositor verification where real Wayland automation cannot yet be automated.
- Macro concurrency behavior may be conservative in v1: a repeated trigger for a still-running macro may be denied or queued, but the chosen behavior must be specified during planning before implementation.
