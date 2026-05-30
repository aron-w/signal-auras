use signal_auras_core::{DiagnosableError, ErrorPhase, ShutdownReason};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventToken(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReadyEvent {
    pub token: RuntimeEventToken,
}

pub struct RuntimeEventLoop {
    poll: mio::Poll,
    events: mio::Events,
}

impl RuntimeEventLoop {
    pub fn new() -> Result<Self, DiagnosableError> {
        Ok(Self {
            poll: mio::Poll::new().map_err(event_loop_error)?,
            events: mio::Events::with_capacity(128),
        })
    }

    pub fn register_readable_fd(
        &mut self,
        fd: RawFd,
        token: RuntimeEventToken,
    ) -> Result<(), DiagnosableError> {
        let mut source = mio::unix::SourceFd(&fd);
        self.poll
            .registry()
            .register(&mut source, mio::Token(token.0), mio::Interest::READABLE)
            .map_err(event_loop_error)
    }

    pub fn wait(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Vec<RuntimeReadyEvent>, DiagnosableError> {
        self.poll
            .poll(&mut self.events, timeout)
            .map_err(event_loop_error)?;
        Ok(self
            .events
            .iter()
            .map(|event| RuntimeReadyEvent {
                token: RuntimeEventToken(event.token().0),
            })
            .collect())
    }
}

fn event_loop_error(error: std::io::Error) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Trigger,
        format!("runtime event loop operation failed: {error}"),
    )
    .with_source("runtime-event-loop")
}

pub struct RuntimeTimerFd {
    timer: nix::sys::timerfd::TimerFd,
}

impl RuntimeTimerFd {
    pub fn new() -> Result<Self, DiagnosableError> {
        let timer = nix::sys::timerfd::TimerFd::new(
            nix::sys::timerfd::ClockId::CLOCK_MONOTONIC,
            nix::sys::timerfd::TimerFlags::TFD_NONBLOCK
                | nix::sys::timerfd::TimerFlags::TFD_CLOEXEC,
        )
        .map_err(runtime_fd_error)?;
        Ok(Self { timer })
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.timer.as_fd().as_raw_fd()
    }

    pub fn arm_after(&self, duration: Duration) -> Result<(), DiagnosableError> {
        let duration = duration.max(Duration::from_millis(1));
        self.timer
            .set(
                nix::sys::timerfd::Expiration::OneShot(nix::sys::time::TimeSpec::from_duration(
                    duration,
                )),
                nix::sys::timerfd::TimerSetTimeFlags::empty(),
            )
            .map_err(runtime_fd_error)
    }

    pub fn disarm(&self) -> Result<(), DiagnosableError> {
        self.timer.unset().map_err(runtime_fd_error)
    }

    pub fn drain(&self) -> Result<(), DiagnosableError> {
        match self.timer.wait() {
            Ok(()) | Err(nix::errno::Errno::EAGAIN) => Ok(()),
            Err(error) => Err(runtime_fd_error(error)),
        }
    }
}

pub struct RuntimeSignalFd {
    signal_fd: nix::sys::signalfd::SignalFd,
    // The blocked mask is kept for the guard lifetime so listener/helper
    // threads spawned afterward inherit shutdown signals as blocked. Dropping
    // the guard restores the current thread's normal delivery path if startup
    // fails before the live runtime loop takes ownership.
    mask: nix::sys::signal::SigSet,
}

#[derive(Debug)]
pub struct RuntimeWakeFd {
    fd: OwnedFd,
}

#[derive(Debug)]
pub struct RuntimeWakeSender {
    fd: OwnedFd,
}

impl RuntimeWakeFd {
    pub fn new() -> Result<Self, DiagnosableError> {
        // Safety: eventfd returns a new owned fd on success. The fd is wrapped
        // in OwnedFd immediately so it is closed exactly once.
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if fd < 0 {
            return Err(runtime_io_error("create eventfd"));
        }
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        })
    }

    pub fn sender(&self) -> Result<RuntimeWakeSender, DiagnosableError> {
        Ok(RuntimeWakeSender {
            fd: self
                .fd
                .as_fd()
                .try_clone_to_owned()
                .map_err(event_loop_error)?,
        })
    }

    pub fn wake(&self) -> Result<(), DiagnosableError> {
        write_eventfd(self.fd.as_raw_fd())
    }

    pub fn drain(&self) -> Result<bool, DiagnosableError> {
        drain_eventfd(self.fd.as_raw_fd())
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl RuntimeWakeSender {
    pub fn wake(&self) -> Result<(), DiagnosableError> {
        write_eventfd(self.fd.as_raw_fd())
    }
}

fn write_eventfd(fd: RawFd) -> Result<(), DiagnosableError> {
    let value = 1u64.to_ne_bytes();
    // Safety: writes a fixed-size u64 to a valid eventfd. The buffer outlives
    // the call and no aliasing requirements are violated.
    let result = unsafe { libc::write(fd, value.as_ptr().cast(), value.len()) };
    if result < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EAGAIN) {
        return Ok(());
    }
    if result < 0 {
        return Err(runtime_io_error("write eventfd"));
    }
    Ok(())
}

fn drain_eventfd(fd: RawFd) -> Result<bool, DiagnosableError> {
    let mut drained = false;
    loop {
        let mut value = [0u8; std::mem::size_of::<u64>()];
        // Safety: reads a fixed-size u64 from a valid eventfd into a stack
        // buffer. The buffer is valid for the duration of the call.
        let result = unsafe { libc::read(fd, value.as_mut_ptr().cast(), value.len()) };
        if result > 0 {
            drained = true;
            continue;
        }
        if result < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EAGAIN) {
            return Ok(drained);
        }
        if result < 0 {
            return Err(runtime_io_error("read eventfd"));
        }
        return Ok(drained);
    }
}

impl RuntimeSignalFd {
    pub fn sigint() -> Result<Self, DiagnosableError> {
        Self::for_signal(nix::sys::signal::Signal::SIGINT)
    }

    pub fn shutdown() -> Result<Self, DiagnosableError> {
        Self::for_signals([
            nix::sys::signal::Signal::SIGINT,
            nix::sys::signal::Signal::SIGTERM,
        ])
    }

    pub fn for_signal(signal: nix::sys::signal::Signal) -> Result<Self, DiagnosableError> {
        Self::for_signals([signal])
    }

    pub fn for_signals(
        signals: impl IntoIterator<Item = nix::sys::signal::Signal>,
    ) -> Result<Self, DiagnosableError> {
        let mut mask = nix::sys::signal::SigSet::empty();
        for signal in signals {
            mask.add(signal);
        }
        nix::sys::signal::sigprocmask(nix::sys::signal::SigmaskHow::SIG_BLOCK, Some(&mask), None)
            .map_err(runtime_fd_error)?;
        let signal_fd = nix::sys::signalfd::SignalFd::with_flags(
            &mask,
            nix::sys::signalfd::SfdFlags::SFD_NONBLOCK | nix::sys::signalfd::SfdFlags::SFD_CLOEXEC,
        )
        .map_err(runtime_fd_error)?;
        Ok(Self { signal_fd, mask })
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.signal_fd.as_raw_fd()
    }

    pub fn drain(&mut self) -> Result<bool, DiagnosableError> {
        Ok(!self.drain_signal_numbers()?.is_empty())
    }

    pub fn drain_shutdown_reason(&mut self) -> Result<Option<ShutdownReason>, DiagnosableError> {
        let mut reason = None;
        for signal in self.drain_signal_numbers()? {
            reason = Some(if signal as i32 == libc::SIGTERM {
                ShutdownReason::SignalTerm
            } else {
                ShutdownReason::CtrlC
            });
        }
        Ok(reason)
    }

    fn drain_signal_numbers(&mut self) -> Result<Vec<u32>, DiagnosableError> {
        let mut signals = Vec::new();
        let mut received = false;
        while let Some(signal) = self.signal_fd.read_signal().map_err(runtime_fd_error)? {
            received = true;
            signals.push(signal.ssi_signo);
        }
        if received {
            Ok(signals)
        } else {
            Ok(Vec::new())
        }
    }
}

impl Drop for RuntimeSignalFd {
    fn drop(&mut self) {
        let _ = nix::sys::signal::sigprocmask(
            nix::sys::signal::SigmaskHow::SIG_UNBLOCK,
            Some(&self.mask),
            None,
        );
    }
}

fn runtime_fd_error(error: nix::errno::Errno) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Trigger,
        format!("runtime fd operation failed: {error}"),
    )
    .with_source("runtime-event-loop")
}

fn runtime_io_error(operation: &str) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Trigger,
        format!(
            "runtime fd operation failed to {operation}: {}",
            std::io::Error::last_os_error()
        ),
    )
    .with_source("runtime-event-loop")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd};

    #[test]
    fn reports_registered_readable_fd() {
        let mut fds = [0; 2];
        // Safety: pipe initializes two owned file descriptors in `fds` on
        // success. They are immediately wrapped in File so Drop closes them.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let mut writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        let reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let mut runtime = RuntimeEventLoop::new().unwrap();
        runtime
            .register_readable_fd(reader.as_raw_fd(), RuntimeEventToken(7))
            .unwrap();

        writer.write_all(b"x").unwrap();

        let events = runtime.wait(Some(Duration::from_millis(50))).unwrap();
        assert!(events
            .iter()
            .any(|event| event.token == RuntimeEventToken(7)));
    }

    #[test]
    fn timer_fd_reports_deadline_readiness() {
        let timer = RuntimeTimerFd::new().unwrap();
        let mut runtime = RuntimeEventLoop::new().unwrap();
        runtime
            .register_readable_fd(timer.as_raw_fd(), RuntimeEventToken(3))
            .unwrap();

        timer.arm_after(Duration::from_millis(1)).unwrap();

        let events = runtime.wait(Some(Duration::from_millis(100))).unwrap();
        assert!(events
            .iter()
            .any(|event| event.token == RuntimeEventToken(3)));
        timer.drain().unwrap();
    }

    #[test]
    fn signal_fd_reports_masked_signal_readiness() {
        let mut signal_fd = RuntimeSignalFd::for_signal(nix::sys::signal::Signal::SIGUSR1).unwrap();
        let mut runtime = RuntimeEventLoop::new().unwrap();
        runtime
            .register_readable_fd(signal_fd.as_raw_fd(), RuntimeEventToken(4))
            .unwrap();

        nix::sys::signal::raise(nix::sys::signal::Signal::SIGUSR1).unwrap();

        let events = runtime.wait(Some(Duration::from_millis(100))).unwrap();
        assert!(events
            .iter()
            .any(|event| event.token == RuntimeEventToken(4)));
        assert!(signal_fd.drain().unwrap());
    }

    #[test]
    fn shutdown_signal_fd_reports_sigint_and_sigterm_reasons() {
        let mut signal_fd = RuntimeSignalFd::shutdown().unwrap();

        nix::sys::signal::raise(nix::sys::signal::Signal::SIGINT).unwrap();
        assert_eq!(
            signal_fd.drain_shutdown_reason().unwrap(),
            Some(ShutdownReason::CtrlC)
        );

        nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();
        assert_eq!(
            signal_fd.drain_shutdown_reason().unwrap(),
            Some(ShutdownReason::SignalTerm)
        );
    }

    #[test]
    fn shutdown_signal_fd_wakes_runtime_event_loop() {
        let mut signal_fd = RuntimeSignalFd::shutdown().unwrap();
        let mut runtime = RuntimeEventLoop::new().unwrap();
        runtime
            .register_readable_fd(signal_fd.as_raw_fd(), RuntimeEventToken(9))
            .unwrap();

        nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();

        let events = runtime.wait(Some(Duration::from_millis(100))).unwrap();
        assert!(events
            .iter()
            .any(|event| event.token == RuntimeEventToken(9)));
        assert_eq!(
            signal_fd.drain_shutdown_reason().unwrap(),
            Some(ShutdownReason::SignalTerm)
        );
        assert_eq!(signal_fd.drain_shutdown_reason().unwrap(), None);
    }

    #[test]
    fn helper_threads_inherit_blocked_shutdown_signal_mask() {
        let _signal_fd = RuntimeSignalFd::shutdown().unwrap();

        let mask = std::thread::spawn(nix::sys::signal::SigSet::thread_get_mask)
            .join()
            .unwrap()
            .unwrap();

        assert!(mask.contains(nix::sys::signal::Signal::SIGINT));
        assert!(mask.contains(nix::sys::signal::Signal::SIGTERM));
    }

    #[test]
    fn wake_fd_reports_cross_thread_readiness_and_drains() {
        let wake_fd = RuntimeWakeFd::new().unwrap();
        let sender = wake_fd.sender().unwrap();
        let mut runtime = RuntimeEventLoop::new().unwrap();
        runtime
            .register_readable_fd(wake_fd.as_raw_fd(), RuntimeEventToken(8))
            .unwrap();

        std::thread::spawn(move || sender.wake().unwrap())
            .join()
            .unwrap();

        let events = runtime.wait(Some(Duration::from_millis(100))).unwrap();
        assert!(events
            .iter()
            .any(|event| event.token == RuntimeEventToken(8)));
        assert!(wake_fd.drain().unwrap());
        assert!(!wake_fd.drain().unwrap());
    }
}
