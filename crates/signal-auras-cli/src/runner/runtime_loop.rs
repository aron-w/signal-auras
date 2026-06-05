use signal_auras_core::{MotionRuntime, MotionTrigger};
use signal_auras_wayland::evdev::{EvdevInputWaitOutcome, ObservedInputEvent};
use std::{collections::BTreeMap, os::fd::RawFd, time::Duration, time::Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeWake {
    Signal,
    Timer,
    Callback,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWakeFds {
    signal_fd: RawFd,
    timer_fd: RawFd,
    callback_fd: Option<RawFd>,
}

#[derive(Debug)]
pub(super) enum RuntimeWaitOutcome {
    Input(ObservedInputEvent),
    Signal,
    Timer,
    Callback,
    Timeout,
    UnknownRuntimeFd,
}

impl RuntimeWakeFds {
    pub(super) fn new(signal_fd: RawFd, timer_fd: RawFd, callback_fd: Option<RawFd>) -> Self {
        Self {
            signal_fd,
            timer_fd,
            callback_fd,
        }
    }

    pub(super) fn poll_fds(self) -> Vec<RawFd> {
        let mut fds = vec![self.signal_fd, self.timer_fd];
        if let Some(callback_fd) = self.callback_fd {
            fds.push(callback_fd);
        }
        fds
    }

    pub(super) fn classify(self, fd: RawFd) -> RuntimeWake {
        if fd == self.signal_fd {
            RuntimeWake::Signal
        } else if fd == self.timer_fd {
            RuntimeWake::Timer
        } else if self.callback_fd == Some(fd) {
            RuntimeWake::Callback
        } else {
            RuntimeWake::Unknown
        }
    }
}

pub(super) fn classify_wait_outcome(
    outcome: EvdevInputWaitOutcome,
    wake_fds: RuntimeWakeFds,
) -> RuntimeWaitOutcome {
    match outcome {
        EvdevInputWaitOutcome::Input(event) => RuntimeWaitOutcome::Input(event),
        EvdevInputWaitOutcome::RuntimeFd(fd) => match wake_fds.classify(fd) {
            RuntimeWake::Signal => RuntimeWaitOutcome::Signal,
            RuntimeWake::Timer => RuntimeWaitOutcome::Timer,
            RuntimeWake::Callback => RuntimeWaitOutcome::Callback,
            RuntimeWake::Unknown => RuntimeWaitOutcome::UnknownRuntimeFd,
        },
        EvdevInputWaitOutcome::Timeout => RuntimeWaitOutcome::Timeout,
    }
}

pub(super) fn next_live_wait_timeout(
    repeat_ticks: &[(MotionTrigger, u64)],
    last_repeat_ticks: &BTreeMap<MotionTrigger, Instant>,
    motion_runtime: &MotionRuntime,
) -> Duration {
    let mut timeout = idle_wait_timeout();
    let now = Instant::now();
    for (trigger, interval_ms) in repeat_ticks {
        if !motion_runtime.loop_is_active(trigger) {
            continue;
        }
        let due_in = match last_repeat_ticks.get(trigger) {
            Some(last_tick) => {
                let interval = Duration::from_millis(*interval_ms);
                interval.saturating_sub(now.saturating_duration_since(*last_tick))
            }
            None => Duration::ZERO,
        };
        timeout = timeout.min(due_in);
    }
    timeout
}

pub(super) fn idle_wait_timeout() -> Duration {
    Duration::from_secs(300)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{
        BindingMode, LoopBody, LoopDefinition, LoopInterval, LoopRepeat, MacroAction,
        MacroDefinition, MotionDefinition, MotionInputEvent, MotionToken, MouseButton,
    };
    use signal_auras_wayland::evdev::{KernelEventTimestamp, RawInputEvent};
    use std::path::PathBuf;

    #[test]
    fn runtime_wake_fds_include_callback_when_available() {
        let fds = RuntimeWakeFds::new(1, 2, Some(3));

        assert_eq!(fds.poll_fds(), vec![1, 2, 3]);
    }

    #[test]
    fn runtime_wake_fds_classify_signal_timer_callback_and_unknown() {
        let fds = RuntimeWakeFds::new(10, 11, Some(12));

        assert_eq!(fds.classify(10), RuntimeWake::Signal);
        assert_eq!(fds.classify(11), RuntimeWake::Timer);
        assert_eq!(fds.classify(12), RuntimeWake::Callback);
        assert_eq!(fds.classify(99), RuntimeWake::Unknown);
    }

    #[test]
    fn classify_wait_outcome_preserves_input_events() {
        let event = ObservedInputEvent {
            raw: RawInputEvent {
                event_type: 1,
                code: 30,
                value: 1,
                kernel_timestamp: KernelEventTimestamp::Unavailable,
            },
            event: Some(MotionInputEvent::pressed(MotionToken::Key("a".to_string()))),
            source: PathBuf::from("/dev/input/event0"),
            grabbed: false,
            observed_at: Instant::now(),
        };

        let outcome = classify_wait_outcome(
            EvdevInputWaitOutcome::Input(event.clone()),
            RuntimeWakeFds::new(1, 2, None),
        );

        match outcome {
            RuntimeWaitOutcome::Input(actual) => {
                assert_eq!(actual.raw.code, event.raw.code);
                assert_eq!(actual.event, event.event);
            }
            other => panic!("expected input outcome, got {other:?}"),
        }
    }

    #[test]
    fn classify_wait_outcome_maps_runtime_fds_and_timeout() {
        let wake_fds = RuntimeWakeFds::new(10, 11, Some(12));

        assert!(matches!(
            classify_wait_outcome(EvdevInputWaitOutcome::RuntimeFd(10), wake_fds),
            RuntimeWaitOutcome::Signal
        ));
        assert!(matches!(
            classify_wait_outcome(EvdevInputWaitOutcome::RuntimeFd(11), wake_fds),
            RuntimeWaitOutcome::Timer
        ));
        assert!(matches!(
            classify_wait_outcome(EvdevInputWaitOutcome::RuntimeFd(12), wake_fds),
            RuntimeWaitOutcome::Callback
        ));
        assert!(matches!(
            classify_wait_outcome(EvdevInputWaitOutcome::RuntimeFd(99), wake_fds),
            RuntimeWaitOutcome::UnknownRuntimeFd
        ));
        assert!(matches!(
            classify_wait_outcome(EvdevInputWaitOutcome::Timeout, wake_fds),
            RuntimeWaitOutcome::Timeout
        ));
    }

    #[test]
    fn idle_wait_timeout_is_long_when_no_runtime_work_is_pending() {
        let motion_runtime = MotionRuntime::new(std::iter::empty::<MotionDefinition>());

        let timeout = next_live_wait_timeout(&[], &BTreeMap::new(), &motion_runtime);

        assert_eq!(timeout, idle_wait_timeout());
        assert_eq!(timeout, Duration::from_secs(300));
    }

    #[test]
    fn active_repeat_without_prior_tick_is_ready_immediately() {
        let trigger = MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap();
        let mut motion_runtime = MotionRuntime::new([repeat_motion(trigger.clone())]);
        motion_runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        motion_runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        motion_runtime.handle_input(MotionInputEvent::released(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        motion_runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        assert!(motion_runtime.loop_is_active(&trigger));

        let timeout = next_live_wait_timeout(&[(trigger, 50)], &BTreeMap::new(), &motion_runtime);

        assert_eq!(timeout, Duration::ZERO);
    }

    fn repeat_motion(trigger: MotionTrigger) -> MotionDefinition {
        let macro_definition = MacroDefinition::new(vec![MacroAction::delay(1).unwrap()]).unwrap();
        let loop_definition = LoopDefinition::new(
            MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
            None,
            LoopBody::Repeat(LoopRepeat::new(
                LoopInterval::new(50).unwrap(),
                macro_definition,
            )),
            None,
        );
        MotionDefinition::new(
            trigger,
            BindingMode::Passthrough,
            None,
            Some(loop_definition),
            signal_auras_core::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap()
    }
}
