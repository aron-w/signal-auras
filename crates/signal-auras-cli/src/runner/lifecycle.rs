use crate::logging::RuntimeLog;
use signal_auras_core::{
    CapabilityReport, ControllerProgram, DiagnosableError, ErrorPhase, HotkeyBinding,
    HotkeyRegistrar, MacroExecutor, RuntimeMotion, RuntimePress, RuntimeStats,
};
use signal_auras_lua::ImperativeLuaController;
use signal_auras_wayland::{event_loop::RuntimeSignalFd, RealWaylandAdapter};

use super::DeveloperDiagnosticRuntime;

pub(super) struct LiveRealLifecycleArgs<'a> {
    pub(super) bindings: &'a [HotkeyBinding],
    pub(super) motions: &'a [RuntimeMotion],
    pub(super) presses: &'a [RuntimePress],
    pub(super) adapter: &'a mut RealWaylandAdapter,
    pub(super) stats: &'a mut RuntimeStats,
    pub(super) log: RuntimeLog,
    pub(super) signal_fd: RuntimeSignalFd,
    pub(super) developer_diagnostics: &'a mut DeveloperDiagnosticRuntime,
}

pub(super) struct LiveRealControllerLifecycleArgs<'a> {
    pub(super) program: &'a ControllerProgram,
    pub(super) runtime: Option<&'a ImperativeLuaController>,
    pub(super) adapter: &'a mut RealWaylandAdapter,
    pub(super) capabilities: &'a CapabilityReport,
    pub(super) stats: &'a mut RuntimeStats,
    pub(super) log: RuntimeLog,
    pub(super) signal_fd: RuntimeSignalFd,
    pub(super) developer_diagnostics: &'a mut DeveloperDiagnosticRuntime,
}

pub(super) struct RealAdapterCleanupSession<'a> {
    adapter: &'a mut RealWaylandAdapter,
    cleaned: bool,
}

impl<'a> RealAdapterCleanupSession<'a> {
    pub(super) fn new(adapter: &'a mut RealWaylandAdapter) -> Self {
        Self {
            adapter,
            cleaned: false,
        }
    }

    pub(super) fn adapter(&mut self) -> &mut RealWaylandAdapter {
        self.adapter
    }

    pub(super) fn cleanup_after_error(
        &mut self,
        phase: ErrorPhase,
    ) -> Result<(), DiagnosableError> {
        if self.cleaned {
            return Ok(());
        }
        self.cleaned = true;
        cleanup_real_adapter_resources(self.adapter, phase, true)
    }

    pub(super) fn finish_normal_shutdown(&mut self) -> Result<(), DiagnosableError> {
        if self.cleaned {
            return Ok(());
        }
        self.cleaned = true;
        cleanup_real_adapter_resources(self.adapter, ErrorPhase::Shutdown, false)
    }
}

pub(super) fn cleanup_after_error(
    registrar: &mut impl HotkeyRegistrar,
    phase: ErrorPhase,
) -> Result<(), DiagnosableError> {
    registrar.unregister_all().map_err(|error| {
        DiagnosableError::new(phase, format!("cleanup failed after runner error: {error}"))
    })
}

fn cleanup_real_adapter_resources(
    adapter: &mut RealWaylandAdapter,
    phase: ErrorPhase,
    after_error: bool,
) -> Result<(), DiagnosableError> {
    tracing::info!(
        event = "cleanup_after_error",
        resource = "adapter_sessions",
        disposition = "started"
    );
    let mut first_error = adapter.cancel_pending().err();
    tracing::info!(
        event = "cleanup_after_error",
        resource = "registrations",
        disposition = "started"
    );
    if let Err(error) = adapter.unregister_all() {
        if first_error.is_none() {
            first_error = Some(error);
        }
    }
    if let Some(error) = first_error {
        tracing::warn!(
            event = "cleanup_after_error",
            after_error,
            disposition = "failed",
            error = %error
        );
        return Err(DiagnosableError::new(
            phase,
            format!("cleanup failed after runner error: {error}"),
        ));
    }
    tracing::info!(
        event = "cleanup_after_error",
        after_error,
        disposition = "completed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{HotkeyBinding, RegistrationId};

    struct CleanupProbe {
        unregister_calls: usize,
        unregister_error: Option<DiagnosableError>,
    }

    impl CleanupProbe {
        fn new() -> Self {
            Self {
                unregister_calls: 0,
                unregister_error: None,
            }
        }

        fn failing(error: DiagnosableError) -> Self {
            Self {
                unregister_calls: 0,
                unregister_error: Some(error),
            }
        }
    }

    impl HotkeyRegistrar for CleanupProbe {
        fn register(
            &mut self,
            _binding: HotkeyBinding,
        ) -> Result<RegistrationId, DiagnosableError> {
            unreachable!("cleanup tests do not register hotkeys")
        }

        fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
            self.unregister_calls += 1;
            match self.unregister_error.clone() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    #[test]
    fn cleanup_after_error_unregisters_once() {
        let mut probe = CleanupProbe::new();

        cleanup_after_error(&mut probe, ErrorPhase::Registration).unwrap();

        assert_eq!(probe.unregister_calls, 1);
    }

    #[test]
    fn cleanup_after_error_reports_requested_phase_when_cleanup_fails() {
        let cleanup_error = DiagnosableError::new(ErrorPhase::Shutdown, "mock unregister failed");
        let mut probe = CleanupProbe::failing(cleanup_error);

        let error = cleanup_after_error(&mut probe, ErrorPhase::Registration).unwrap_err();

        assert_eq!(probe.unregister_calls, 1);
        assert_eq!(error.phase, ErrorPhase::Registration);
        assert!(error.message.contains("cleanup failed after runner error"));
        assert!(error.message.contains("mock unregister failed"));
    }

    #[test]
    fn real_adapter_cleanup_session_is_idempotent_for_empty_adapter() {
        let mut adapter = RealWaylandAdapter::new();
        let mut session = RealAdapterCleanupSession::new(&mut adapter);

        session
            .cleanup_after_error(ErrorPhase::Registration)
            .unwrap();
        session.cleanup_after_error(ErrorPhase::Shutdown).unwrap();
        session.finish_normal_shutdown().unwrap();
    }
}
