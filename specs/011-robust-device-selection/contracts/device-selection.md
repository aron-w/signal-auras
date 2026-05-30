# Contract: Evdev Device Selection

## Selected Devices

- Opening explicit selected paths MUST consider only the configured paths.
- Duplicate selected paths MUST be diagnosed and deduplicated before observation.
- Missing, unreadable, permission-denied, unsupported, and self-generated paths
  MUST produce diagnosable skipped or blocking errors.
- If no selected path can be used, opening MUST fail closed with a
  no-usable-devices diagnostic.
- Reappearing selected paths MAY be reopened during the current run, but the
  provider MUST NOT broaden to unrelated paths.

## `devices = "all"`

- Startup MUST scan current `/dev/input/event*` candidates for this run only.
- Unreadable, unsupported, noisy, permission-denied, duplicate, and
  self-generated candidates MUST be skipped and reported.
- Startup MUST succeed when at least one eligible readable candidate exists.
- Startup MUST fail closed when no usable candidate exists.
- Hotplug MUST add newly eligible devices and mark removed/unreadable devices
  inactive without persisting discovered state.

## Noise and Read Failures

- Unsupported events MUST return no decoded motion event and MUST NOT prevent
  subsequent supported events from being processed.
- Device removal or `ENODEV` MUST mark the device inactive and report removal.
- Other read failures MUST become diagnosable trigger errors.
