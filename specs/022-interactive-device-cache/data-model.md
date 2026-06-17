# Data Model: Interactive Device Cache

## Interactive Device Selection

- **Fields**: backend, mode, output, selection kind.
- **Validation**: Only valid for evdev providers. Resolved before live adapter
  startup into explicit selected paths.
- **State transitions**: declared -> valid-cache -> selected paths, or declared
  -> prompt -> cached selected paths, or declared -> fail-closed.

## Canonical Main Lua Path

- **Fields**: original CLI path, canonical absolute path, cache key.
- **Validation**: Must resolve before interactive cache lookup.
- **Relationship**: Owns exactly one runtime device cache file.

## Device Identity Fingerprint

- **Fields**: event path, device name, physical path, unique id, bus/vendor/
  product/version identifiers when available.
- **Validation**: Current fingerprint must match cached fingerprint before a
  cached event path is accepted.

## Runtime Device Cache Entry

- **Fields**: format version, canonical script path, provider mode, provider
  output, selected device entries, creation/update timestamp.
- **Validation**: Script path, provider mode/output, selected device paths,
  device identities, permissions, and own-device exclusion must all pass.

## Cache Validation Result

- **Statuses**: accepted, missing, stale, invalid, permission-incomplete,
  unsafe-runtime-dir, cancelled.
- **Rules**: Accepted becomes strict selected paths. Any other status prompts
  only in interactive terminal startup; non-interactive startup fails closed.

## Permission Repair Attempt

- **Fields**: selected evdev paths, uinput required flag, user confirmation,
  command result.
- **Validation**: Runs only after explicit confirmation and must be revalidated.

## Interactive Device Diagnostic Report

- **Fields**: runtime cache path, cache status, selected path statuses,
  fingerprint status, permission status, remediation.
- **Rules**: Read-only. Does not prompt, grant permissions, observe input, or
  rewrite cache.
