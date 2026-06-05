use super::{runtime_loop::idle_wait_timeout, ControllerHost};
use signal_auras_core::{
    queue_controller_callback_outputs, CallbackDisposition, CapabilityReport, CapabilitySet,
    ControllerProgram, DiagnosableError, ErrorPhase, LuaCallbackScheduler, RuntimeStats,
    RustOperationBatch, SynthesizedInputRequest,
};
use signal_auras_lua::{
    ImperativeLuaController, LuaCallbackStep, LuaHostRequest, LuaHostResponse, LuaLogLevel,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

struct PendingControllerCallback {
    task: signal_auras_core::LuaCallbackTask,
    coroutine: signal_auras_lua::LuaCallbackCoroutine,
    response: LuaHostResponse,
    ready_at: Instant,
    execution_elapsed: Duration,
}

#[derive(Default)]
pub(super) struct ControllerCallbackContinuations {
    pending: VecDeque<PendingControllerCallback>,
}

impl ControllerCallbackContinuations {
    fn push(
        &mut self,
        task: signal_auras_core::LuaCallbackTask,
        coroutine: signal_auras_lua::LuaCallbackCoroutine,
        response: LuaHostResponse,
        ready_at: Instant,
        execution_elapsed: Duration,
    ) {
        self.pending.push_back(PendingControllerCallback {
            task,
            coroutine,
            response,
            ready_at,
            execution_elapsed,
        });
    }

    fn pop_ready(&mut self) -> Option<PendingControllerCallback> {
        let now = Instant::now();
        let index = self
            .pending
            .iter()
            .position(|pending| pending.ready_at <= now)?;
        self.pending.remove(index)
    }

    pub(super) fn next_wait_timeout(&self) -> Duration {
        let now = Instant::now();
        self.pending
            .iter()
            .map(|pending| pending.ready_at.saturating_duration_since(now))
            .min()
            .unwrap_or_else(idle_wait_timeout)
    }

    pub(super) fn force_ready(&mut self) {
        let now = Instant::now();
        for pending in &mut self.pending {
            pending.ready_at = now;
        }
    }

    pub(super) fn cancel_all(&mut self, scheduler: &mut LuaCallbackScheduler) -> usize {
        let mut cancelled = 0;
        while let Some(pending) = self.pending.pop_front() {
            if scheduler.cancel_task(&pending.task) == CallbackDisposition::Cancelled {
                cancelled += 1;
            }
        }
        cancelled
    }
}

enum ControllerTaskExecution {
    Complete,
    PendingSleep {
        coroutine: signal_auras_lua::LuaCallbackCoroutine,
        response: LuaHostResponse,
        ready_at: Instant,
    },
}

pub(super) fn drain_controller_callbacks<E>(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    scheduler: &mut LuaCallbackScheduler,
    continuations: &mut ControllerCallbackContinuations,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: ControllerHost,
{
    while let Some(mut pending) = continuations.pop_ready() {
        let started_at = Instant::now();
        let result = resume_imperative_controller_task(
            runtime.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "controller callback '{}' has no imperative runtime",
                        pending.task.callback
                    ),
                )
            })?,
            pending.coroutine,
            pending.response,
            capabilities,
            executor,
            stats,
        );
        pending.execution_elapsed += started_at.elapsed();
        match result? {
            ControllerTaskExecution::Complete => {
                let disposition = scheduler.finish(pending.task, pending.execution_elapsed);
                if disposition == CallbackDisposition::Slow {
                    tracing::warn!(event = "callback_received", disposition = "slow");
                }
            }
            ControllerTaskExecution::PendingSleep {
                coroutine,
                response,
                ready_at,
            } => continuations.push(
                pending.task,
                coroutine,
                response,
                ready_at,
                pending.execution_elapsed,
            ),
        }
    }
    while let Some(task) = scheduler.pop_next() {
        let started_at = Instant::now();
        let result =
            execute_controller_task(program, runtime, &task, capabilities, executor, stats);
        let execution_elapsed = started_at.elapsed();
        match result? {
            ControllerTaskExecution::Complete => {
                let disposition = scheduler.finish(task, execution_elapsed);
                if disposition == CallbackDisposition::Slow {
                    tracing::warn!(event = "callback_received", disposition = "slow");
                }
            }
            ControllerTaskExecution::PendingSleep {
                coroutine,
                response,
                ready_at,
            } => {
                continuations.push(task, coroutine, response, ready_at, execution_elapsed);
            }
        }
    }
    Ok(())
}

fn execute_controller_task<E>(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    task: &signal_auras_core::LuaCallbackTask,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<ControllerTaskExecution, DiagnosableError>
where
    E: ControllerHost,
{
    let callback = program.callback(&task.callback).ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("controller callback '{}' is unavailable", task.callback),
        )
    })?;
    if callback.actions.is_empty() {
        if let Some(runtime) = runtime {
            return execute_imperative_controller_task(
                runtime,
                &task.callback,
                capabilities,
                executor,
                stats,
            );
        }
    }
    let mut batch = RustOperationBatch::new(256)?;
    queue_controller_callback_outputs(callback, capabilities, &mut batch)?;
    stats.record_output_queue_depth(batch.len() as u64);
    for request in batch.drain() {
        match executor.execute_input_request(request)? {
            signal_auras_core::InputEmission::Emitted => stats.record_synthesized_input_emitted(),
            signal_auras_core::InputEmission::Denied => {
                stats.record_synthesized_input_denied();
                return Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input was denied",
                ));
            }
            signal_auras_core::InputEmission::Failed => {
                return Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input failed",
                ));
            }
            signal_auras_core::InputEmission::Cancelled => {
                return Err(DiagnosableError::new(
                    ErrorPhase::Shutdown,
                    "controller synthesized input was cancelled",
                ));
            }
        }
    }
    stats.macro_success_count += 1;
    Ok(ControllerTaskExecution::Complete)
}

fn execute_imperative_controller_task<E>(
    runtime: &ImperativeLuaController,
    callback_name: &str,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<ControllerTaskExecution, DiagnosableError>
where
    E: ControllerHost,
{
    let coroutine = runtime.start_callback(callback_name)?;
    resume_imperative_controller_task(
        runtime,
        coroutine,
        LuaHostResponse::Unit,
        capabilities,
        executor,
        stats,
    )
}

fn resume_imperative_controller_task<E>(
    runtime: &ImperativeLuaController,
    coroutine: signal_auras_lua::LuaCallbackCoroutine,
    mut response: LuaHostResponse,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<ControllerTaskExecution, DiagnosableError>
where
    E: ControllerHost,
{
    let declared = runtime.registrations().required_capabilities();
    loop {
        match runtime.resume_callback(&coroutine, response, declared)? {
            LuaCallbackStep::Complete => {
                stats.macro_success_count += 1;
                return Ok(ControllerTaskExecution::Complete);
            }
            LuaCallbackStep::Yielded(request) => {
                if let Some(required) = request.required_capability() {
                    let required = CapabilitySet::new([required]);
                    if let Some(error) = capabilities.first_blocking_error(&required) {
                        stats.record_permission_failure();
                        stats.denied_action_count += 1;
                        return Err(error);
                    }
                }
                if let LuaHostRequest::Sleep { duration_ms } = request {
                    let duration = Duration::from_millis(duration_ms);
                    return Ok(ControllerTaskExecution::PendingSleep {
                        coroutine,
                        response: LuaHostResponse::Unit,
                        ready_at: Instant::now() + duration,
                    });
                }
                response = execute_lua_host_request(request, executor, stats)?;
            }
        }
    }
}

fn execute_lua_host_request<E>(
    request: LuaHostRequest,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<LuaHostResponse, DiagnosableError>
where
    E: ControllerHost,
{
    match request {
        LuaHostRequest::Sleep { duration_ms } => {
            executor.sleep(Duration::from_millis(duration_ms))?;
            Ok(LuaHostResponse::Unit)
        }
        LuaHostRequest::Log { level, message } => {
            match level {
                LuaLogLevel::Debug => tracing::debug!(event = "lua_log", message = %message),
                LuaLogLevel::Info => tracing::info!(event = "lua_log", message = %message),
                LuaLogLevel::Warn => tracing::warn!(event = "lua_log", message = %message),
            }
            Ok(LuaHostResponse::Unit)
        }
        LuaHostRequest::ActiveWindow { include_title } => executor
            .active_window(include_title)
            .map(LuaHostResponse::ActiveWindow),
        LuaHostRequest::FindWindow { processes } => executor
            .find_window(&processes)
            .map(LuaHostResponse::WindowHandle),
        LuaHostRequest::ActivateWindow { handle } => {
            executor.activate_window(&handle).map(LuaHostResponse::Bool)
        }
        LuaHostRequest::WaitActive { handle, timeout_ms } => executor
            .wait_active_window(&handle, Duration::from_millis(timeout_ms))
            .map(LuaHostResponse::Bool),
        LuaHostRequest::Input { action } => {
            let request = SynthesizedInputRequest::new(action, 0);
            match executor.execute_input_request(request)? {
                signal_auras_core::InputEmission::Emitted => {
                    stats.record_synthesized_input_emitted();
                    Ok(LuaHostResponse::Unit)
                }
                signal_auras_core::InputEmission::Denied => {
                    stats.record_synthesized_input_denied();
                    Err(DiagnosableError::new(
                        ErrorPhase::MacroExecution,
                        "controller synthesized input was denied",
                    ))
                }
                signal_auras_core::InputEmission::Failed => Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input failed",
                )),
                signal_auras_core::InputEmission::Cancelled => Err(DiagnosableError::new(
                    ErrorPhase::Shutdown,
                    "controller synthesized input was cancelled",
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{InputEmission, MacroAction, MacroExecutor};

    #[derive(Default)]
    struct RecordingHost {
        sleep_calls: usize,
        input_calls: usize,
        emission: Option<InputEmission>,
    }

    impl MacroExecutor for RecordingHost {
        fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
            self.input_calls += 1;
            Ok(())
        }

        fn execute_input_request(
            &mut self,
            _request: SynthesizedInputRequest,
        ) -> Result<InputEmission, DiagnosableError> {
            self.input_calls += 1;
            Ok(self.emission.unwrap_or(InputEmission::Emitted))
        }
    }

    impl ControllerHost for RecordingHost {
        fn sleep(&mut self, _duration: Duration) -> Result<(), DiagnosableError> {
            self.sleep_calls += 1;
            Ok(())
        }
    }

    #[test]
    fn host_sleep_request_uses_controller_host_sleep() {
        let mut host = RecordingHost::default();
        let mut stats = RuntimeStats::new();

        execute_lua_host_request(
            LuaHostRequest::Sleep { duration_ms: 25 },
            &mut host,
            &mut stats,
        )
        .unwrap();

        assert_eq!(host.sleep_calls, 1);
        assert_eq!(host.input_calls, 0);
    }

    #[test]
    fn host_input_request_records_emitted_output() {
        let mut host = RecordingHost::default();
        let mut stats = RuntimeStats::new();

        execute_lua_host_request(
            LuaHostRequest::Input {
                action: MacroAction::delay(1).unwrap(),
            },
            &mut host,
            &mut stats,
        )
        .unwrap();

        assert_eq!(host.input_calls, 1);
        assert_eq!(stats.synthesized_input_emitted_count, 1);
        assert_eq!(stats.synthesized_input_denied_count, 0);
    }

    #[test]
    fn host_input_request_records_denied_output_and_fails_closed() {
        let mut host = RecordingHost {
            emission: Some(InputEmission::Denied),
            ..RecordingHost::default()
        };
        let mut stats = RuntimeStats::new();

        let error = execute_lua_host_request(
            LuaHostRequest::Input {
                action: MacroAction::delay(1).unwrap(),
            },
            &mut host,
            &mut stats,
        )
        .unwrap_err();

        assert_eq!(error.phase, ErrorPhase::MacroExecution);
        assert_eq!(stats.synthesized_input_emitted_count, 0);
        assert_eq!(stats.synthesized_input_denied_count, 1);
    }
}
