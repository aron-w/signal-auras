# Research: Robust Device Selection

## Decision: Keep Selection Policy Inside the Evdev Adapter

**Rationale**: `EvdevObservationProvider` already owns device opening,
nonblocking reads, udev readiness, rescan state, grab state, and own-uinput
device rejection. Hardening selected paths, broad discovery, and reopen behavior
there keeps unsafe fd behavior behind one Rust boundary.

**Alternatives considered**: Moving selection into the CLI would duplicate
adapter behavior and make hotplug/reopen harder to test. A daemon or persistent
cache would violate the current-run-only requirement.

## Decision: Strict Selected Paths, Tolerant `devices = "all"`

**Rationale**: Explicit selected paths are the least-privilege daily mode and
must fail closed when none can be used. Broad discovery is already explicit
consent, so unreadable or unsupported candidates can be skipped as long as at
least one eligible readable device remains and skipped candidates are reported.

**Alternatives considered**: Falling back from selected paths to discovery would
silently broaden observation. Failing `devices = "all"` on the first bad device
would make broad discovery unusable on normal Linux desktops.

## Decision: Use Existing File-Based Test Fixtures

**Rationale**: The existing evdev tests use temporary files that contain encoded
`input_event` records. They exercise selection, event decoding, fairness, and
reopen behavior without requiring real `/dev/input` hardware or compositor
permissions.

**Alternatives considered**: Real evdev/uinput integration tests require root or
system groups and compositor state, so they remain manual/supplemental.

## Decision: Doctor Diagnostics Stay Read-Only

**Rationale**: `doctor input` should explain the configured selected paths,
permission problems, own-device exclusion, skipped broad discovery limitations,
and stable path recommendations without opening a live observation session or
granting capabilities.

**Alternatives considered**: A live probe could provide more detail but risks
turning diagnostics into input observation. The command remains a read-only
configuration and access check.
