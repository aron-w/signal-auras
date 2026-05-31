use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const MOTION_LATENCY_BUCKETS_MS: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 250, u64::MAX];
const MOTION_EVENT_AGE_BUCKETS_MS: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 250, u64::MAX];
const CALLBACK_LATENCY_BUCKETS_MS: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 250, u64::MAX];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    CtrlC,
    SignalTerm,
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
    pub capability_probe_success_count: u64,
    pub capability_probe_failure_count: u64,
    pub shortcut_event_ignored_count: u64,
    pub callback_event_received_count: u64,
    pub callback_event_dispatched_count: u64,
    pub callback_event_dropped_count: u64,
    pub max_callback_dispatch_latency_ms: u64,
    callback_dispatch_latency_total_ms: u64,
    callback_dispatch_latency_buckets: [u64; CALLBACK_LATENCY_BUCKETS_MS.len()],
    pub active_process_match_count: u64,
    pub active_process_non_match_count: u64,
    pub metadata_unavailable_count: u64,
    pub synthesized_input_emitted_count: u64,
    pub synthesized_input_denied_count: u64,
    pub consumed_trigger_event_count: u64,
    pub passthrough_trigger_event_count: u64,
    pub motion_input_event_count: u64,
    pub motion_repeat_tick_count: u64,
    pub motion_repeat_skipped_count: u64,
    pub motion_repeat_cancel_count: u64,
    pub motion_discard_count: u64,
    pub non_repeat_trigger_skipped_count: u64,
    pub max_motion_dispatch_latency_ms: u64,
    motion_dispatch_latency_total_ms: u64,
    motion_dispatch_latency_buckets: [u64; MOTION_LATENCY_BUCKETS_MS.len()],
    pub motion_event_age_sample_count: u64,
    pub motion_event_age_unavailable_count: u64,
    pub max_motion_event_age_ms: u64,
    motion_event_age_total_ms: u64,
    motion_event_age_buckets: [u64; MOTION_EVENT_AGE_BUCKETS_MS.len()],
    pub event_loop_wakeup_count: u64,
    pub hotplug_add_count: u64,
    pub hotplug_remove_count: u64,
    pub output_queue_failure_count: u64,
    pub cancelled_macro_run_count: u64,
    pub max_output_queue_depth: u64,
    pub kde_bridge_setup_count: u64,
    pub kde_bridge_cleanup_count: u64,
    pub cleanup_success_count: u64,
    pub cleanup_failure_count: u64,
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
            capability_probe_success_count: 0,
            capability_probe_failure_count: 0,
            shortcut_event_ignored_count: 0,
            callback_event_received_count: 0,
            callback_event_dispatched_count: 0,
            callback_event_dropped_count: 0,
            max_callback_dispatch_latency_ms: 0,
            callback_dispatch_latency_total_ms: 0,
            callback_dispatch_latency_buckets: [0; CALLBACK_LATENCY_BUCKETS_MS.len()],
            active_process_match_count: 0,
            active_process_non_match_count: 0,
            metadata_unavailable_count: 0,
            synthesized_input_emitted_count: 0,
            synthesized_input_denied_count: 0,
            consumed_trigger_event_count: 0,
            passthrough_trigger_event_count: 0,
            motion_input_event_count: 0,
            motion_repeat_tick_count: 0,
            motion_repeat_skipped_count: 0,
            motion_repeat_cancel_count: 0,
            motion_discard_count: 0,
            non_repeat_trigger_skipped_count: 0,
            max_motion_dispatch_latency_ms: 0,
            motion_dispatch_latency_total_ms: 0,
            motion_dispatch_latency_buckets: [0; MOTION_LATENCY_BUCKETS_MS.len()],
            motion_event_age_sample_count: 0,
            motion_event_age_unavailable_count: 0,
            max_motion_event_age_ms: 0,
            motion_event_age_total_ms: 0,
            motion_event_age_buckets: [0; MOTION_EVENT_AGE_BUCKETS_MS.len()],
            event_loop_wakeup_count: 0,
            hotplug_add_count: 0,
            hotplug_remove_count: 0,
            output_queue_failure_count: 0,
            cancelled_macro_run_count: 0,
            max_output_queue_depth: 0,
            kde_bridge_setup_count: 0,
            kde_bridge_cleanup_count: 0,
            cleanup_success_count: 0,
            cleanup_failure_count: 0,
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

    pub fn record_capability_probe_success(&mut self) {
        self.capability_probe_success_count += 1;
    }

    pub fn record_capability_probe_failure(&mut self) {
        self.capability_probe_failure_count += 1;
    }

    pub fn record_callback_received(&mut self) {
        self.callback_event_received_count += 1;
    }

    pub fn record_callback_dispatched(&mut self, dispatch_latency_ms: u64) {
        self.callback_event_dispatched_count += 1;
        self.max_callback_dispatch_latency_ms = self
            .max_callback_dispatch_latency_ms
            .max(dispatch_latency_ms);
        self.callback_dispatch_latency_total_ms += dispatch_latency_ms;
        let bucket = CALLBACK_LATENCY_BUCKETS_MS
            .iter()
            .position(|upper_bound| dispatch_latency_ms <= *upper_bound)
            .unwrap_or(CALLBACK_LATENCY_BUCKETS_MS.len() - 1);
        self.callback_dispatch_latency_buckets[bucket] += 1;
    }

    pub fn record_callback_dropped(&mut self, count: u64) {
        self.callback_event_dropped_count += count;
    }

    pub fn record_shortcut_ignored(&mut self) {
        self.shortcut_event_ignored_count += 1;
    }

    pub fn average_callback_dispatch_latency_ms(&self) -> u64 {
        if self.callback_event_dispatched_count == 0 {
            return 0;
        }
        self.callback_dispatch_latency_total_ms / self.callback_event_dispatched_count
    }

    pub fn callback_dispatch_latency_p95_ms(&self) -> u64 {
        self.callback_dispatch_latency_percentile_ms(95)
    }

    pub fn callback_dispatch_latency_p99_ms(&self) -> u64 {
        self.callback_dispatch_latency_percentile_ms(99)
    }

    fn callback_dispatch_latency_percentile_ms(&self, percentile: u64) -> u64 {
        if self.callback_event_dispatched_count == 0 {
            return 0;
        }
        let rank = (self.callback_event_dispatched_count * percentile).div_ceil(100);
        let mut seen = 0;
        for (bucket, upper_bound) in self
            .callback_dispatch_latency_buckets
            .iter()
            .zip(CALLBACK_LATENCY_BUCKETS_MS)
        {
            seen += *bucket;
            if seen >= rank {
                return upper_bound;
            }
        }
        self.max_callback_dispatch_latency_ms
    }

    pub fn record_active_process_match(&mut self) {
        self.active_process_match_count += 1;
    }

    pub fn record_active_process_non_match(&mut self) {
        self.active_process_non_match_count += 1;
    }

    pub fn record_metadata_unavailable(&mut self) {
        self.metadata_unavailable_count += 1;
    }

    pub fn record_synthesized_input_emitted(&mut self) {
        self.synthesized_input_emitted_count += 1;
    }

    pub fn record_synthesized_input_denied(&mut self) {
        self.synthesized_input_denied_count += 1;
    }

    pub fn record_consumed_trigger_event(&mut self) {
        self.consumed_trigger_event_count += 1;
    }

    pub fn record_passthrough_trigger_event(&mut self) {
        self.passthrough_trigger_event_count += 1;
    }

    pub fn record_motion_input_event(&mut self, dispatch_latency_ms: u64) {
        self.motion_input_event_count += 1;
        self.max_motion_dispatch_latency_ms =
            self.max_motion_dispatch_latency_ms.max(dispatch_latency_ms);
        self.motion_dispatch_latency_total_ms += dispatch_latency_ms;
        let bucket = MOTION_LATENCY_BUCKETS_MS
            .iter()
            .position(|upper_bound| dispatch_latency_ms <= *upper_bound)
            .unwrap_or(MOTION_LATENCY_BUCKETS_MS.len() - 1);
        self.motion_dispatch_latency_buckets[bucket] += 1;
    }

    pub fn average_motion_dispatch_latency_ms(&self) -> u64 {
        if self.motion_input_event_count == 0 {
            return 0;
        }
        self.motion_dispatch_latency_total_ms / self.motion_input_event_count
    }

    pub fn motion_dispatch_latency_p95_ms(&self) -> u64 {
        self.motion_dispatch_latency_percentile_ms(95)
    }

    pub fn motion_dispatch_latency_p99_ms(&self) -> u64 {
        self.motion_dispatch_latency_percentile_ms(99)
    }

    fn motion_dispatch_latency_percentile_ms(&self, percentile: u64) -> u64 {
        if self.motion_input_event_count == 0 {
            return 0;
        }
        let rank = (self.motion_input_event_count * percentile).div_ceil(100);
        let mut seen = 0;
        for (bucket, upper_bound) in self
            .motion_dispatch_latency_buckets
            .iter()
            .zip(MOTION_LATENCY_BUCKETS_MS)
        {
            seen += *bucket;
            if seen >= rank {
                return upper_bound;
            }
        }
        self.max_motion_dispatch_latency_ms
    }

    pub fn record_motion_event_age(&mut self, event_age_ms: u64) {
        self.motion_event_age_sample_count += 1;
        self.max_motion_event_age_ms = self.max_motion_event_age_ms.max(event_age_ms);
        self.motion_event_age_total_ms += event_age_ms;
        let bucket = MOTION_EVENT_AGE_BUCKETS_MS
            .iter()
            .position(|upper_bound| event_age_ms <= *upper_bound)
            .unwrap_or(MOTION_EVENT_AGE_BUCKETS_MS.len() - 1);
        self.motion_event_age_buckets[bucket] += 1;
    }

    pub fn record_motion_event_age_unavailable(&mut self) {
        self.motion_event_age_unavailable_count += 1;
    }

    pub fn average_motion_event_age_ms(&self) -> u64 {
        if self.motion_event_age_sample_count == 0 {
            return 0;
        }
        self.motion_event_age_total_ms / self.motion_event_age_sample_count
    }

    pub fn motion_event_age_p95_ms(&self) -> u64 {
        self.motion_event_age_percentile_ms(95)
    }

    pub fn motion_event_age_p99_ms(&self) -> u64 {
        self.motion_event_age_percentile_ms(99)
    }

    fn motion_event_age_percentile_ms(&self, percentile: u64) -> u64 {
        if self.motion_event_age_sample_count == 0 {
            return 0;
        }
        let rank = (self.motion_event_age_sample_count * percentile).div_ceil(100);
        let mut seen = 0;
        for (bucket, upper_bound) in self
            .motion_event_age_buckets
            .iter()
            .zip(MOTION_EVENT_AGE_BUCKETS_MS)
        {
            seen += *bucket;
            if seen >= rank {
                return upper_bound;
            }
        }
        self.max_motion_event_age_ms
    }

    pub fn record_motion_repeat_tick(&mut self) {
        self.motion_repeat_tick_count += 1;
    }

    pub fn record_motion_repeat_skipped(&mut self, count: u64) {
        self.motion_repeat_skipped_count += count;
    }

    pub fn record_motion_repeat_cancel(&mut self) {
        self.motion_repeat_cancel_count += 1;
    }

    pub fn record_motion_discard(&mut self) {
        self.motion_discard_count += 1;
    }

    pub fn record_non_repeat_trigger_skipped(&mut self) {
        self.non_repeat_trigger_skipped_count += 1;
    }

    pub fn record_event_loop_wakeup(&mut self) {
        self.event_loop_wakeup_count += 1;
    }

    pub fn record_hotplug_add(&mut self) {
        self.hotplug_add_count += 1;
    }

    pub fn record_hotplug_remove(&mut self) {
        self.hotplug_remove_count += 1;
    }

    pub fn record_output_queue_failure(&mut self) {
        self.output_queue_failure_count += 1;
    }

    pub fn record_cancelled_macro_runs(&mut self, count: u64) {
        self.cancelled_macro_run_count += count;
    }

    pub fn record_output_queue_depth(&mut self, depth: u64) {
        self.max_output_queue_depth = self.max_output_queue_depth.max(depth);
    }

    pub fn record_kde_bridge_setup(&mut self) {
        self.kde_bridge_setup_count += 1;
    }

    pub fn record_kde_bridge_cleanup(&mut self) {
        self.kde_bridge_cleanup_count += 1;
    }

    pub fn record_cleanup_success(&mut self) {
        self.cleanup_success_count += 1;
    }

    pub fn record_cleanup_failure(&mut self) {
        self.cleanup_failure_count += 1;
    }

    pub fn render_summary(&self, reason: ShutdownReason) -> String {
        format!(
            "final_summary reason={reason:?} elapsed_ms={} triggers={} successes={} failures={} denials={} permission_failures={} scope_mismatches={} capability_probe_successes={} capability_probe_failures={} ignored_events={} callbacks_received={} callbacks_dispatched={} callbacks_dropped={} avg_callback_dispatch_latency_ms={} p95_callback_dispatch_latency_ms={} p99_callback_dispatch_latency_ms={} max_callback_dispatch_latency_ms={} active_process_matches={} active_process_non_matches={} metadata_unavailable={} input_emitted={} input_denied={} consumed_events={} passthrough_events={} motion_inputs={} repeat_ticks={} repeat_skipped_or_coalesced={} repeat_cancels={} motion_discards={} non_repeat_skipped_or_denied={} avg_motion_dispatch_latency_ms={} p95_motion_dispatch_latency_ms={} p99_motion_dispatch_latency_ms={} max_motion_dispatch_latency_ms={} motion_event_age_samples={} motion_event_age_unavailable={} avg_motion_event_age_ms={} p95_motion_event_age_ms={} p99_motion_event_age_ms={} max_motion_event_age_ms={} event_loop_wakeups={} hotplug_adds={} hotplug_removes={} output_queue_failures={} cancelled_macro_runs={} max_output_queue_depth={} kde_bridge_setups={} kde_bridge_cleanups={} cleanup_successes={} cleanup_failures={}",
            self.elapsed_runtime().as_millis(),
            self.total_triggers(),
            self.macro_success_count,
            self.macro_failure_count,
            self.denied_action_count,
            self.permission_failure_count,
            self.scope_mismatch_count,
            self.capability_probe_success_count,
            self.capability_probe_failure_count,
            self.shortcut_event_ignored_count,
            self.callback_event_received_count,
            self.callback_event_dispatched_count,
            self.callback_event_dropped_count,
            self.average_callback_dispatch_latency_ms(),
            self.callback_dispatch_latency_p95_ms(),
            self.callback_dispatch_latency_p99_ms(),
            self.max_callback_dispatch_latency_ms,
            self.active_process_match_count,
            self.active_process_non_match_count,
            self.metadata_unavailable_count,
            self.synthesized_input_emitted_count,
            self.synthesized_input_denied_count,
            self.consumed_trigger_event_count,
            self.passthrough_trigger_event_count,
            self.motion_input_event_count,
            self.motion_repeat_tick_count,
            self.motion_repeat_skipped_count,
            self.motion_repeat_cancel_count,
            self.motion_discard_count,
            self.non_repeat_trigger_skipped_count,
            self.average_motion_dispatch_latency_ms(),
            self.motion_dispatch_latency_p95_ms(),
            self.motion_dispatch_latency_p99_ms(),
            self.max_motion_dispatch_latency_ms,
            self.motion_event_age_sample_count,
            self.motion_event_age_unavailable_count,
            self.average_motion_event_age_ms(),
            self.motion_event_age_p95_ms(),
            self.motion_event_age_p99_ms(),
            self.max_motion_event_age_ms,
            self.event_loop_wakeup_count,
            self.hotplug_add_count,
            self.hotplug_remove_count,
            self.output_queue_failure_count,
            self.cancelled_macro_run_count,
            self.max_output_queue_depth,
            self.kde_bridge_setup_count,
            self.kde_bridge_cleanup_count,
            self.cleanup_success_count,
            self.cleanup_failure_count
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
            .render_summary(ShutdownReason::SignalTerm)
            .contains("SignalTerm"));
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

    #[test]
    fn motion_latency_metrics_report_average_p95_and_p99() {
        let mut stats = RuntimeStats::new();
        for _ in 0..95 {
            stats.record_motion_input_event(20);
        }
        for _ in 0..4 {
            stats.record_motion_input_event(50);
        }
        stats.record_motion_input_event(100);

        assert_eq!(stats.motion_input_event_count, 100);
        assert_eq!(stats.average_motion_dispatch_latency_ms(), 22);
        assert_eq!(stats.motion_dispatch_latency_p95_ms(), 20);
        assert_eq!(stats.motion_dispatch_latency_p99_ms(), 50);
        assert_eq!(stats.max_motion_dispatch_latency_ms, 100);

        let summary = stats.render_summary(ShutdownReason::CtrlC);
        assert!(summary.contains("avg_motion_dispatch_latency_ms=22"));
        assert!(summary.contains("p95_motion_dispatch_latency_ms=20"));
        assert!(summary.contains("p99_motion_dispatch_latency_ms=50"));
    }

    #[test]
    fn motion_event_age_metrics_report_average_p95_p99_and_unavailable() {
        let mut stats = RuntimeStats::new();
        for _ in 0..95 {
            stats.record_motion_event_age(20);
        }
        for _ in 0..4 {
            stats.record_motion_event_age(50);
        }
        stats.record_motion_event_age(100);
        stats.record_motion_event_age_unavailable();
        stats.record_motion_event_age_unavailable();

        assert_eq!(stats.motion_event_age_sample_count, 100);
        assert_eq!(stats.motion_event_age_unavailable_count, 2);
        assert_eq!(stats.average_motion_event_age_ms(), 22);
        assert_eq!(stats.motion_event_age_p95_ms(), 20);
        assert_eq!(stats.motion_event_age_p99_ms(), 50);
        assert_eq!(stats.max_motion_event_age_ms, 100);

        let summary = stats.render_summary(ShutdownReason::CtrlC);
        assert!(summary.contains("motion_event_age_samples=100"));
        assert!(summary.contains("motion_event_age_unavailable=2"));
        assert!(summary.contains("avg_motion_event_age_ms=22"));
        assert!(summary.contains("p95_motion_event_age_ms=20"));
        assert!(summary.contains("p99_motion_event_age_ms=50"));
        assert!(summary.contains("max_motion_event_age_ms=100"));
    }

    #[test]
    fn dispatch_and_true_event_age_labels_are_distinct() {
        let mut stats = RuntimeStats::new();
        stats.record_motion_input_event(1);
        stats.record_motion_event_age(50);

        let summary = stats.render_summary(ShutdownReason::CtrlC);

        assert!(summary.contains("avg_motion_dispatch_latency_ms=1"));
        assert!(summary.contains("avg_motion_event_age_ms=50"));
    }

    #[test]
    fn callback_latency_metrics_report_average_p95_p99_and_drops() {
        let mut stats = RuntimeStats::new();
        for _ in 0..95 {
            stats.record_callback_received();
            stats.record_callback_dispatched(20);
        }
        for _ in 0..4 {
            stats.record_callback_received();
            stats.record_callback_dispatched(50);
        }
        stats.record_callback_received();
        stats.record_callback_dispatched(100);
        stats.record_callback_dropped(7);

        assert_eq!(stats.callback_event_received_count, 100);
        assert_eq!(stats.callback_event_dispatched_count, 100);
        assert_eq!(stats.callback_event_dropped_count, 7);
        assert_eq!(stats.average_callback_dispatch_latency_ms(), 22);
        assert_eq!(stats.callback_dispatch_latency_p95_ms(), 20);
        assert_eq!(stats.callback_dispatch_latency_p99_ms(), 50);
        assert_eq!(stats.max_callback_dispatch_latency_ms, 100);

        let summary = stats.render_summary(ShutdownReason::CtrlC);
        assert!(summary.contains("callbacks_received=100"));
        assert!(summary.contains("callbacks_dispatched=100"));
        assert!(summary.contains("callbacks_dropped=7"));
        assert!(summary.contains("avg_callback_dispatch_latency_ms=22"));
        assert!(summary.contains("p95_callback_dispatch_latency_ms=20"));
        assert!(summary.contains("p99_callback_dispatch_latency_ms=50"));
    }

    #[test]
    fn repeat_overload_counters_are_rendered_in_summary() {
        let mut stats = RuntimeStats::new();
        stats.record_motion_repeat_tick();
        stats.record_motion_repeat_skipped(3);
        stats.record_motion_repeat_cancel();
        stats.record_cancelled_macro_runs(1);

        let summary = stats.render_summary(ShutdownReason::CtrlC);

        assert!(summary.contains("repeat_ticks=1"));
        assert!(summary.contains("repeat_skipped_or_coalesced=3"));
        assert!(summary.contains("repeat_cancels=1"));
        assert!(summary.contains("cancelled_macro_runs=1"));
    }

    #[test]
    fn non_repeat_collision_counters_are_rendered_in_summary() {
        let mut stats = RuntimeStats::new();
        stats.record_non_repeat_trigger_skipped();

        let summary = stats.render_summary(ShutdownReason::CtrlC);

        assert!(summary.contains("non_repeat_skipped_or_denied=1"));
    }
}
