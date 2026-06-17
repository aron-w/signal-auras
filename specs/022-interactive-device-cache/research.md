# Research: Interactive Device Cache

## Decision: Use `$XDG_RUNTIME_DIR` Only

**Rationale**: The requested cache is runtime-local and mandatory per main Lua
path. `$XDG_RUNTIME_DIR/signal-auras/input-devices/` gives user ownership,
session cleanup, and avoids hidden persistent state.

**Alternatives considered**: `$XDG_CACHE_HOME` or project-local files would
survive sessions and weaken revocation expectations.

## Decision: Derive Cache Key From Canonical Main Lua Path

**Rationale**: Canonical paths prevent accidental duplicate cache files when the
same script is invoked from different working directories. A hash keeps cache
file names filesystem-safe and private enough for normal diagnostics.

**Alternatives considered**: User-provided cache ids were rejected because the
user clarified the cache is not optional and must exist for every main Lua path.

## Decision: Terminal Checklist for v1

**Rationale**: KDE Plasma portals do not provide a generic immediate UI that
selects `/dev/input/event*` devices and grants evdev ACLs to a terminal
process. A terminal checklist is predictable for beginner developers running
the CLI directly.

**Alternatives considered**: Plasma dialogs and RemoteDesktop/InputCapture
portals were rejected for v1 because they do not model this backend's direct
evdev path selection and selected ACL repair flow.

## Decision: Selected-Device ACL Repair

**Rationale**: `just unsafe-input-acl` grants broad event access for local tests.
The interactive flow should instead run a small sudo/setfacl command scoped to
the selected evdev paths and `/dev/uinput` when configured.

**Alternatives considered**: Running the broad helper automatically was
rejected because it grants more authority than the user selected.

## Decision: Custom Runtime Cache Format

**Rationale**: The cache is small, line-oriented, and internal to one runtime
directory. Avoiding a new serialization dependency preserves the existing
minimal dependency posture.

**Alternatives considered**: Adding JSON/TOML serialization crates was rejected
because the feature does not need a general persistent config format.
