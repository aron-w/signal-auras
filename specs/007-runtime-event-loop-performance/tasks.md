# Tasks: Runtime Event Loop Performance

**Input**: Design documents from `/specs/007-runtime-event-loop-performance/`

- [x] T001 Add runtime/logging dependencies to Cargo metadata and Nix shell.
- [x] T002 Add incremental macro-run state in the core crate.
- [x] T003 Add scheduler tests for nonblocking delay and cancellation.
- [x] T004 Wire live runner motion/hotkey macro execution through queued macro
  runs instead of blocking sleeps.
- [x] T005 Add structured tracing initialization for `--verbose` diagnostics.
- [x] T006 Replace evdev hotplug diagnostics with tracing.
- [x] T007 Add event-loop readiness primitive and fd readiness test.
- [x] T008 Batch uinput writes per logical action.
- [x] T009 Add bounded runtime summary counters for event-loop/output/cancel
  diagnostics.
- [x] T010 Complete follow-up migration from periodic all-device rescan to udev
  monitor events wired into the live loop.
- [x] T011 Complete follow-up migration from Ctrl-C atomic polling and repeat
  `Instant` checks to signalfd/timerfd registered in the event loop.
- [x] T012 Run full Nix verification commands and record results.
