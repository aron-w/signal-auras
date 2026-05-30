# Research: Callback Wakeups

## Decision: Use a pollable wake fd for KDE callback delivery

Rationale: The live runner already waits on evdev, timerfd, and signalfd style
readiness. A callback wake fd lets the KWin D-Bus listener wake the same loop
without periodic short sleeps or waiting for unrelated input.

Alternatives considered: Polling the callback channel on a fixed interval was
rejected because it creates latency jitter and idle churn. Adding an async
runtime was rejected because the existing runtime is already a small fd-driven
loop and the constitution favors minimal composition.

## Decision: Bound accepted callback events with newest-drop semantics

Rationale: A fixed-size FIFO queue preserves arrival order for accepted
callbacks and gives burst behavior a clear disposition when the limit is
reached. Dropping the newest event keeps earlier accepted callbacks ordered and
prevents unbounded memory growth.

Alternatives considered: Unbounded channels were rejected because burst losses
would be invisible and memory could grow without limit. Coalescing callbacks was
rejected because separate shortcut activations can represent separate user
intent.

## Decision: Measure callback-to-dispatch latency at runner dispatch decision

Rationale: The KWin listener records callback receipt time, and the live runner
records latency when it accepts, denies, or ignores the callback. This matches
the spec's user-visible timing target without exposing implementation details in
the public requirements.

Alternatives considered: Measuring only D-Bus method latency was rejected
because it misses runner scheduling delay. Measuring macro completion was
rejected because macro content and output provider behavior are separate
concerns.

## Decision: Keep callback-triggered macro behavior on existing consent paths

Rationale: Callback shortcuts use the same configured hotkey binding,
process-scope decision, macro queue, and synthesized input execution model as
existing shortcut-triggered macros. No new Lua syntax or ambient capability is
introduced.

Alternatives considered: A dedicated callback binding API was rejected because
the feature is a runtime wakeup hardening change, not a scripting surface
change.
