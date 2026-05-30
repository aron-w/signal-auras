# Data Model: Full Keyboard Key Coverage

## Canonical Key Token

Represents one stable user-visible key identity.

**Fields**:
- `name`: canonical Lua/user-facing token, for example `Enter`, `PageUp`, `KPEnter`, `VolumeUp`
- `evdev_code`: standard Linux key code when the token maps to a normal evdev key event
- `category`: letter, number, punctuation, modifier, function, navigation, editing, keypad, system, media, or unknown
- `aliases`: accepted legacy or familiar spellings that normalize to this token

**Validation**:
- Canonical names are unique.
- Aliases are unique across all tokens after case and whitespace normalization.
- Existing aliases listed in the spec remain accepted.

## Key Alias

Represents a non-canonical accepted spelling.

**Fields**:
- `alias`: user-provided spelling
- `canonical_name`: target canonical token
- `compatibility_reason`: legacy API, one-character shorthand, generated Linux spelling, or familiar synonym

**Validation**:
- Alias lookup must be deterministic.
- Alias-equivalent triggers must compare equal for duplicate detection.

## Observed Key Event

Represents a current-run physical input event decoded from a provider.

**Fields**:
- `device_path_or_status`: selected path, discovered path, or status when path is unavailable
- `raw_code`: Linux event code
- `state`: pressed, released, repeated, or unsupported value
- `canonical_token`: token when raw code maps to a known standard key
- `timestamp_availability`: existing kernel timestamp availability state when provided by evdev
- `unavailable_reason`: unknown code, vendor-specific code, non-key event, permission denial, or provider unsupported

**Validation**:
- Unknown raw codes preserve `raw_code` and do not invent `canonical_token`.
- Observed event data remains current-run only.

## Trigger Support Status

Represents whether a canonical key can trigger automation through the selected provider.

**Fields**:
- `canonical_token`
- `surface`: leader, motion trigger, repeat hold token, structured binding key, or legacy hotkey
- `provider`: evdev, KDE shortcut, portal, or unavailable provider
- `status`: supported, unsupported, denied, unavailable, or unknown until observed
- `reason`: provider limitation, permission denial, missing device, unobserved hardware behavior, or unsupported code

## Output Support Status

Represents whether a canonical key can be emitted by macro output.

**Fields**:
- `canonical_token`
- `backend`: uinput, portal, KDE shortcut output adapter, or unavailable backend
- `status`: supported, unsupported, denied, unavailable
- `reason`: backend limitation, permission denial, missing `/dev/uinput`, unsupported key code, or revoked access

## Discovery Diagnostic Report

Represents one explicit key discovery run.

**Fields**:
- `run_scope`: current-run command invocation only
- `input_provider_status`: selected devices, broad discovery status, permission status, or unavailable provider
- `observed_keys`: sequence of observed key events and support statuses
- `unobserved_notes`: user-visible notes for hardware controls that produced no event during discovery
- `persistence`: always none

**State Transitions**:
- `not_started` -> `active` after explicit command and consent checks
- `active` -> `completed` on normal exit
- `active` -> `failed_closed` on permission/provider/device denial
- Any terminal state drops observed key state
