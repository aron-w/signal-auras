# Runtime Event Loop Contract

The runtime event loop exposes a reusable Rust API that can register readable
file descriptors and wait until either an fd is ready or a deadline expires.

The live runner uses this primitive family as the integration point for evdev
fds, udev monitor fds, timerfd repeat deadlines, signalfd shutdown, and output
queue wakeups.

The event loop must not own privileged behavior directly; it only reports
readiness tokens to the adapter/runtime layer that owns capability checks.
