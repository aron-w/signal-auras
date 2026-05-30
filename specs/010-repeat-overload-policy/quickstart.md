# Quickstart: Repeat Overload Policy

## Automated Verification

Run the standard checks from the repository root:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

When Nix is unavailable in the execution environment, use the closest local equivalents:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Expected Coverage

- Slow repeat output produces skipped/coalesced ticks instead of overlapping output.
- A long-held repeat survives at least 10,000 due tick opportunities without unbounded pending work.
- Cancellation release prevents all later repeat output for the cancelled hold.
- Two held repeat bindings maintain independent overload and cancellation counters.
- Verbose and final diagnostics report executed, skipped/coalesced, and cancelled repeat counts without macro payloads.
- Existing Lua repeat configurations load without migration.

## Manual Runtime Spot Check

1. Start the runner with an explicit unsafe evdev/uinput motion config and `--verbose`.
2. Use a repeat macro whose action duration exceeds its repeat interval.
3. Hold the configured input long enough to trigger overload.
4. Release the hold and confirm no further repeat output begins after the release is processed.
5. Confirm final diagnostics include repeat tick, skipped/coalesced, cancellation, and cancelled queued-run counts.
