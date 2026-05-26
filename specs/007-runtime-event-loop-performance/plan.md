# Implementation Plan: Runtime Event Loop Performance

**Branch**: `007-runtime-event-loop-performance`
**Date**: 2026-05-27
**Spec**: [spec.md](./spec.md)

## Summary

Upgrade the live runner from blocking macro execution and ad hoc diagnostics to
an incremental runtime model. The implementation adds core macro-run state,
structured tracing, batched uinput output, udev hotplug readiness, timerfd and
signalfd wakeups, and bounded runtime counters while preserving the current Lua
API.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake.

**Primary Dependencies**: `mio`, `nix`, `udev`, `tracing`, and
`tracing-subscriber`, plus existing Wayland/KDE/portal crates.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland.

**Testing**: `nix develop -c cargo fmt --check`,
`nix develop -c cargo clippy --all-targets -- -D warnings`,
`nix develop -c cargo test`, and `nix flake check`.

## Constitution Check

- Library-First: PASS. Macro-run and readiness behavior are Rust APIs with
  tests before CLI wiring.
- Wayland/Compositor Awareness: PASS. Runtime scope remains KDE Wayland and
  unsafe evdev/uinput opt-in.
- Rust Safety Boundaries: PASS. fd readiness and uinput writes remain isolated
  in the Wayland adapter.
- Lua Extension Contract: PASS. No Lua API change.
- NixOS Reproducibility: PASS. New native/system dependencies are represented
  in `flake.nix`.
- Security and Consent: PASS. No new ambient capability.
- Testable Automation Behavior: PASS. Core scheduler and fd readiness tests are
  automated.
- Minimal Composition: PASS. No daemon, IPC, persistence, or async runtime.
- No Hidden Global Behavior: PASS. All input/output remains current-run only.
- Incremental Delivery: PASS. This delivers scheduler, logging, output,
  udev, timerfd, and signalfd hardening as a separable runtime increment.

## Complexity Tracking

The feature intentionally adds event-loop/logging dependencies. This is
justified because the previous polling-only hardening cannot make macro delays,
shutdown, timers, and hotplug share one efficient readiness model.
