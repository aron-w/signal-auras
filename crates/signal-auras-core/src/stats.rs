use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    CtrlC,
    StartupError,
    RuntimeError,
}

#[derive(Debug)]
pub struct RuntimeStats {
    started_at: Instant,
    pub registration_attempts: u64,
    pub registration_successes: u64,
    pub registration_failures: u64,
    pub trigger_count_by_hotkey: BTreeMap<String, u64>,
    pub macro_success_count: u64,
    pub macro_failure_count: u64,
    pub denied_action_count: u64,
    pub permission_failure_count: u64,
    pub scope_mismatch_count: u64,
}

impl Default for RuntimeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeStats {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            registration_attempts: 0,
            registration_successes: 0,
            registration_failures: 0,
            trigger_count_by_hotkey: BTreeMap::new(),
            macro_success_count: 0,
            macro_failure_count: 0,
            denied_action_count: 0,
            permission_failure_count: 0,
            scope_mismatch_count: 0,
        }
    }

    pub fn elapsed_runtime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn record_trigger(&mut self, hotkey: &str) {
        *self
            .trigger_count_by_hotkey
            .entry(hotkey.to_string())
            .or_default() += 1;
    }

    pub fn total_triggers(&self) -> u64 {
        self.trigger_count_by_hotkey.values().sum()
    }

    pub fn record_registration_attempt(&mut self) {
        self.registration_attempts += 1;
    }

    pub fn record_registration_success(&mut self) {
        self.registration_successes += 1;
    }

    pub fn record_registration_failure(&mut self) {
        self.registration_failures += 1;
    }

    pub fn record_permission_failure(&mut self) {
        self.permission_failure_count += 1;
    }

    pub fn render_summary(&self, reason: ShutdownReason) -> String {
        format!(
            "final_summary reason={reason:?} elapsed_ms={} triggers={} successes={} failures={} denials={} permission_failures={} scope_mismatches={}",
            self.elapsed_runtime().as_millis(),
            self.total_triggers(),
            self.macro_success_count,
            self.macro_failure_count,
            self.denied_action_count,
            self.permission_failure_count,
            self.scope_mismatch_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_at_zero() {
        let stats = RuntimeStats::new();
        assert_eq!(stats.macro_success_count, 0);
        assert_eq!(stats.trigger_count_by_hotkey.values().sum::<u64>(), 0);
    }

    #[test]
    fn summary_includes_shutdown_reason() {
        let stats = RuntimeStats::new();
        assert!(stats
            .render_summary(ShutdownReason::CtrlC)
            .contains("CtrlC"));
    }

    #[test]
    fn shutdown_summaries_cover_ctrl_c_startup_and_runtime_reasons() {
        let stats = RuntimeStats::new();

        assert!(stats
            .render_summary(ShutdownReason::CtrlC)
            .contains("CtrlC"));
        assert!(stats
            .render_summary(ShutdownReason::StartupError)
            .contains("StartupError"));
        assert!(stats
            .render_summary(ShutdownReason::RuntimeError)
            .contains("RuntimeError"));
    }

    #[test]
    fn in_flight_shutdown_summary_preserves_partial_runtime_stats() {
        let mut stats = RuntimeStats::new();
        stats.record_registration_attempt();
        stats.record_registration_success();
        stats.record_trigger("F5");
        stats.denied_action_count += 1;
        stats.scope_mismatch_count += 1;
        stats.macro_failure_count += 1;
        stats.record_permission_failure();

        let summary = stats.render_summary(ShutdownReason::CtrlC);

        assert!(summary.contains("triggers=1"));
        assert!(summary.contains("failures=1"));
        assert!(summary.contains("denials=1"));
        assert!(summary.contains("permission_failures=1"));
        assert!(summary.contains("scope_mismatches=1"));
    }
}
