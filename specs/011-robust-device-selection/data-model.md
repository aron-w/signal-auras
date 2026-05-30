# Data Model: Robust Device Selection

## Selected Device Path

- **Fields**: configured path, symlink target if available, normalized identity
  when opened, eligibility status, diagnostic reason.
- **Validation**: Must be explicitly configured by Lua. Duplicate selected paths
  are diagnosed. Missing, unreadable, permission-denied, unsupported, or
  self-generated selected paths are never replaced by unrelated devices.
- **State transitions**: configured -> usable, skipped, inactive, reopened.

## Discovered Device Candidate

- **Fields**: discovered `/dev/input/event*` path, open result, eligibility
  status, skipped reason, active/inactive state.
- **Validation**: Considered only when `devices = "all"` is explicitly
  configured for the current run. Unusable candidates are skipped and reported.
- **State transitions**: discovered -> active, skipped, removed, reopened.

## Device Eligibility Result

- **Statuses**: usable, missing, unreadable, permission-denied, unsupported,
  duplicate, self-generated, removed, reopened, noisy.
- **Rules**: Selected mode fails closed when no selected usable device remains.
  Broad mode starts when at least one usable candidate exists.

## Own Virtual Output Device

- **Fields**: evdev device name, path, relationship to current uinput output.
- **Validation**: Any device named as Signal Auras' virtual output is rejected
  during both explicit selection and broad discovery.

## Doctor Diagnostic Report

- **Fields**: configured provider, selected path statuses, broad discovery
  warning/status, uinput status, stable path recommendations, remediation lines,
  overall ok/failed result.
- **Rules**: Read-only. Does not enable observation, persist discovered paths, or
  grant permissions.
