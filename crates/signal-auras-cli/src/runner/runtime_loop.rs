use signal_auras_core::{MotionRuntime, MotionTrigger};
use std::{collections::BTreeMap, time::Duration, time::Instant};

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
