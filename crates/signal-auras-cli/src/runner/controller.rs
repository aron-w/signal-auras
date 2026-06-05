use super::{runtime_loop::idle_wait_timeout, ControllerHost};
use signal_auras_core::{
    queue_controller_callback_outputs, CallbackDisposition, CapabilityReport, CapabilitySet,
    ControllerProgram, DiagnosableError, ErrorPhase, LuaCallbackScheduler, RuntimeStats,
    RustOperationBatch, SynthesizedInputRequest,
};
use signal_auras_lua::{
    ImperativeLuaController, LuaCallbackStep, LuaExecutionBudget, LuaHostRequest, LuaHostResponse,
    LuaLogLevel,
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
    Complete {
        execution_elapsed: Duration,
    },
    Preempted {
        execution_elapsed: Duration,
    },
    PendingSleep {
        coroutine: signal_auras_lua::LuaCallbackCoroutine,
        response: LuaHostResponse,
        ready_at: Instant,
        execution_elapsed: Duration,
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
            pending
                .task
                .budget
                .saturating_sub(pending.execution_elapsed),
        );
        match result? {
            ControllerTaskExecution::Complete { execution_elapsed } => {
                pending.execution_elapsed += execution_elapsed;
                let disposition = scheduler.finish(pending.task, pending.execution_elapsed);
                if disposition == CallbackDisposition::Slow {
                    tracing::warn!(event = "callback_received", disposition = "slow");
                }
            }
            ControllerTaskExecution::Preempted { execution_elapsed } => {
                pending.execution_elapsed += execution_elapsed;
                record_preempted_callback(
                    scheduler,
                    &pending.task,
                    pending.execution_elapsed,
                    stats,
                );
            }
            ControllerTaskExecution::PendingSleep {
                coroutine,
                response,
                ready_at,
                execution_elapsed,
            } => continuations.push(
                pending.task,
                coroutine,
                response,
                ready_at,
                pending.execution_elapsed + execution_elapsed,
            ),
        }
    }
    while let Some(task) = scheduler.pop_next() {
        let result =
            execute_controller_task(program, runtime, &task, capabilities, executor, stats);
        match result? {
            ControllerTaskExecution::Complete { execution_elapsed } => {
                let disposition = scheduler.finish(task, execution_elapsed);
                if disposition == CallbackDisposition::Slow {
                    tracing::warn!(event = "callback_received", disposition = "slow");
                }
            }
            ControllerTaskExecution::Preempted { execution_elapsed } => {
                record_preempted_callback(scheduler, &task, execution_elapsed, stats);
            }
            ControllerTaskExecution::PendingSleep {
                coroutine,
                response,
                ready_at,
                execution_elapsed,
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
    let started_at = Instant::now();
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
                task,
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
    Ok(ControllerTaskExecution::Complete {
        execution_elapsed: started_at.elapsed(),
    })
}

fn execute_imperative_controller_task<E>(
    runtime: &ImperativeLuaController,
    task: &signal_auras_core::LuaCallbackTask,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<ControllerTaskExecution, DiagnosableError>
where
    E: ControllerHost,
{
    let coroutine = runtime.start_callback(&task.callback)?;
    resume_imperative_controller_task(
        runtime,
        coroutine,
        LuaHostResponse::Unit,
        capabilities,
        executor,
        stats,
        task.budget,
    )
}

fn resume_imperative_controller_task<E>(
    runtime: &ImperativeLuaController,
    coroutine: signal_auras_lua::LuaCallbackCoroutine,
    mut response: LuaHostResponse,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
    mut remaining_budget: Duration,
) -> Result<ControllerTaskExecution, DiagnosableError>
where
    E: ControllerHost,
{
    let declared = runtime.registrations().required_capabilities();
    let mut active_elapsed = Duration::ZERO;
    loop {
        if remaining_budget.is_zero() {
            return Ok(ControllerTaskExecution::Preempted {
                execution_elapsed: active_elapsed,
            });
        }
        let budget = LuaExecutionBudget::with_default_hook_interval(remaining_budget)?;
        let resume_started_at = Instant::now();
        let step = runtime.resume_callback_with_budget(&coroutine, response, declared, budget);
        let resume_elapsed = resume_started_at.elapsed();
        active_elapsed += resume_elapsed;
        remaining_budget = remaining_budget.saturating_sub(resume_elapsed);
        match step? {
            LuaCallbackStep::Complete => {
                stats.macro_success_count += 1;
                return Ok(ControllerTaskExecution::Complete {
                    execution_elapsed: active_elapsed,
                });
            }
            LuaCallbackStep::Preempted => {
                return Ok(ControllerTaskExecution::Preempted {
                    execution_elapsed: active_elapsed,
                });
            }
            LuaCallbackStep::Yielded(request) => {
                if remaining_budget.is_zero() {
                    return Ok(ControllerTaskExecution::Preempted {
                        execution_elapsed: active_elapsed,
                    });
                }
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
                        execution_elapsed: active_elapsed,
                    });
                }
                response = execute_lua_host_request(request, executor, stats)?;
            }
        }
    }
}

fn record_preempted_callback(
    scheduler: &mut LuaCallbackScheduler,
    task: &signal_auras_core::LuaCallbackTask,
    execution_elapsed: Duration,
    stats: &mut RuntimeStats,
) {
    let disposition = scheduler.preempt_task(task);
    stats.record_lua_callback_preempted();
    stats.macro_failure_count += 1;
    tracing::warn!(
        event = "callback_received",
        trigger = %task.registration_label,
        callback = %task.callback,
        disposition = ?disposition,
        budget_ms = task.budget.as_millis(),
        elapsed_ms = execution_elapsed.as_millis(),
        queue_depth = scheduler.pending_len()
    );
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
    use signal_auras_core::{
        available_capability_report, InputEmission, MacroAction, MacroExecutor,
    };

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

    #[test]
    fn drain_callbacks_preempts_runaway_imperative_callback_and_releases_scheduler() {
        let source = r#"
        sa.hotkey({
          trigger = "F5",
          capabilities = { "global_shortcut", "synthesized_input" },
          callback = "spin",
        })

        sa.callback("spin", function()
          while true do
          end
          sa.input.text("after")
        end)
        "#;
        let program = signal_auras_lua::load_lua_controller_program_source(source).unwrap();
        let runtime = ImperativeLuaController::load_source(source).unwrap();
        let registration = program.registrations().registrations()[0].clone();
        let capabilities = available_capability_report(program.required_capabilities(), "test");
        let mut scheduler = LuaCallbackScheduler::new(2, Duration::from_millis(1)).unwrap();
        let mut continuations = ControllerCallbackContinuations::default();
        let mut host = RecordingHost::default();
        let mut stats = RuntimeStats::new();

        assert_eq!(
            scheduler
                .schedule(&registration, &capabilities, Instant::now())
                .disposition,
            CallbackDisposition::Accepted
        );
        drain_controller_callbacks(
            &program,
            Some(&runtime),
            &mut scheduler,
            &mut continuations,
            &capabilities,
            &mut host,
            &mut stats,
        )
        .unwrap();

        assert_eq!(host.input_calls, 0);
        assert_eq!(stats.macro_success_count, 0);
        assert_eq!(stats.lua_callback_preempted_count, 1);
        assert_eq!(
            scheduler
                .schedule(&registration, &capabilities, Instant::now())
                .disposition,
            CallbackDisposition::Accepted
        );
    }

    #[test]
    fn drain_callbacks_preempts_before_executing_request_after_budget_exhaustion() {
        let source = r#"
        sa.hotkey({
          trigger = "F5",
          capabilities = { "global_shortcut", "synthesized_input" },
          callback = "late_output",
        })

        sa.callback("late_output", function()
          local sum = 0
          for i = 1, 500 do
            sum = sum + i
          end
          sa.input.text("after")
        end)
        "#;
        let program = signal_auras_lua::load_lua_controller_program_source(source).unwrap();
        let runtime = ImperativeLuaController::load_source(source).unwrap();
        let registration = program.registrations().registrations()[0].clone();
        let capabilities = available_capability_report(program.required_capabilities(), "test");
        let mut scheduler = LuaCallbackScheduler::new(2, Duration::from_nanos(1)).unwrap();
        let mut continuations = ControllerCallbackContinuations::default();
        let mut host = RecordingHost::default();
        let mut stats = RuntimeStats::new();

        assert_eq!(
            scheduler
                .schedule(&registration, &capabilities, Instant::now())
                .disposition,
            CallbackDisposition::Accepted
        );
        drain_controller_callbacks(
            &program,
            Some(&runtime),
            &mut scheduler,
            &mut continuations,
            &capabilities,
            &mut host,
            &mut stats,
        )
        .unwrap();

        assert_eq!(host.input_calls, 0);
        assert_eq!(stats.macro_success_count, 0);
        assert_eq!(stats.lua_callback_preempted_count, 1);
    }
}
