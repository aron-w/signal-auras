# Quickstart: Scoped Focus Pass-Through

## Automated Verification

Run the standard checks:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

If Nix sandbox evaluation is unavailable, run the closest local `cargo` commands and record the exact Nix failure.

## Scenario: Inactive Outside Scope

1. Configure a script with `scope = { processes = { "kate" } }` and a hotkey or motion.
2. Simulate or focus a non-matching process such as `konsole`.
3. Trigger the configured input.
4. Verify no macro output, no repeat start, no consumed/prevented scoped input, and original input pass-through.

## Scenario: Active After Matching Focus

1. Start inactive with non-matching focus metadata.
2. Provide fresh matching focus metadata for `kate`.
3. Trigger the same configured input.
4. Verify macro or repeat behavior proceeds under existing consent/capability rules.

## Scenario: Deactivation Cancels Work

1. Start a process-scoped repeat or delayed macro while focus matches.
2. Change focus metadata to a non-matching, stale, denied, unavailable, ambiguous, or untrusted state.
3. Verify scoped repeat state and queued scoped macro output are cancelled before further output.

## Scenario: Transition Logs

1. Simulate inactive-to-active and active-to-inactive focus transitions.
2. Verify exactly one info-level `scoped_focus_transition` log per state change.
3. Verify logs include configured rule, state, reason, and freshness context where available without command lines, window titles, or macro text.
