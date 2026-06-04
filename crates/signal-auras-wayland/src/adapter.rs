use signal_auras_core::{
    ActiveProcessContext, ActiveProcessProvider, Capability, CapabilityKind, CapabilityReport,
    CapabilitySet, CapabilityStatus, CleanupReport, DiagnosableError, ErrorPhase, HotkeyBinding,
    HotkeyRegistrar, InputEmission, InputProviderBackend, InputProviderConfig, MacroAction,
    MacroExecutor, MotionInputEvent, MotionToken, OverlayProviderReport, OverlaySnapshot,
    ProcessName, RegistrationId, ScreenSample, ScreenSampleProvider, SynthesizedInputRequest,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::capability::{environment_probe, KdeEnvironment};
use crate::diagnostics::unsupported_protocol;
use crate::overlay::OverlayRendererAdapter;

#[derive(Default)]
pub struct MockableWaylandAdapter {
    registrations: Vec<RegistrationId>,
    active_process: Option<ProcessName>,
    capability_report: CapabilityReport,
    emitted_inputs: Vec<MacroAction>,
}

impl MockableWaylandAdapter {
    pub fn with_active_process(active_process: Option<ProcessName>) -> Self {
        Self {
            registrations: Vec::new(),
            active_process,
            capability_report: CapabilityReport::default(),
            emitted_inputs: Vec::new(),
        }
    }

    pub fn with_capability_report(mut self, capability_report: CapabilityReport) -> Self {
        self.capability_report = capability_report;
        self
    }

    pub fn probe_capabilities(&self, _required: &CapabilitySet) -> CapabilityReport {
        self.capability_report.clone()
    }

    pub fn emitted_inputs(&self) -> &[MacroAction] {
        &self.emitted_inputs
    }
}

impl ActiveProcessProvider for MockableWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(self.active_process.clone())
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        match self.active_process.clone() {
            Some(process) => Ok(ActiveProcessContext::name_only(process)),
            None => Ok(ActiveProcessContext::unavailable(
                "active process metadata is unavailable",
            )),
        }
    }
}

impl HotkeyRegistrar for MockableWaylandAdapter {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        let id = RegistrationId::new(format!("mock-{}", binding.trigger_label()));
        self.registrations.push(id.clone());
        Ok(id)
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.registrations.clear();
        Ok(())
    }
}

impl MacroExecutor for MockableWaylandAdapter {
    fn execute_action(&mut self, action: &MacroAction) -> Result<(), DiagnosableError> {
        self.emitted_inputs.push(action.clone());
        Ok(())
    }

    fn execute_input_request(
        &mut self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        self.execute_action(&request.action)?;
        Ok(InputEmission::Emitted)
    }

    fn cancel_pending(&mut self) -> Result<(), DiagnosableError> {
        Ok(())
    }
}

pub fn real_registration_unavailable() -> DiagnosableError {
    unsupported_protocol(Capability::GlobalShortcut)
}

pub struct RealWaylandAdapter {
    registrations: Vec<RegistrationId>,
    environment: Option<KdeEnvironment>,
    rejected_hotkeys: BTreeSet<String>,
    portal_session: Option<crate::portal::PortalInputSession>,
    screen_cast_session: Option<crate::portal::PortalScreenCastSession>,
    uinput_session: Option<crate::uinput::UinputOutputSession>,
    shortcut_bridge: Option<crate::kde_bridge::KwinShortcutBridge>,
    evdev_provider: Option<crate::evdev::EvdevObservationProvider>,
    overlay_renderer: crate::overlay::NativeOverlayRenderer,
    overlay_placements: BTreeMap<String, crate::overlay::OverlayWindowPlacement>,
    overlay_placement_attempts: BTreeMap<String, (crate::overlay::OverlayWindowPlacement, u8)>,
}

impl RealWaylandAdapter {
    pub fn new() -> Self {
        Self {
            registrations: Vec::new(),
            environment: None,
            rejected_hotkeys: BTreeSet::new(),
            portal_session: None,
            screen_cast_session: None,
            uinput_session: None,
            shortcut_bridge: None,
            evdev_provider: None,
            overlay_renderer: crate::overlay::NativeOverlayRenderer::live(),
            overlay_placements: BTreeMap::new(),
            overlay_placement_attempts: BTreeMap::new(),
        }
    }

    pub fn from_environment(environment: KdeEnvironment) -> Self {
        Self {
            registrations: Vec::new(),
            environment: Some(environment),
            rejected_hotkeys: BTreeSet::new(),
            portal_session: None,
            screen_cast_session: None,
            uinput_session: None,
            shortcut_bridge: None,
            evdev_provider: None,
            overlay_renderer: crate::overlay::NativeOverlayRenderer::in_memory(),
            overlay_placements: BTreeMap::new(),
            overlay_placement_attempts: BTreeMap::new(),
        }
    }

    pub fn reject_hotkey_for_test(&mut self, hotkey: impl Into<String>) {
        self.rejected_hotkeys.insert(hotkey.into());
    }

    pub fn ensure_active_process_provider(&mut self) -> Result<(), DiagnosableError> {
        if self.environment.is_some() {
            return Ok(());
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        self.shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .ensure_active_process_monitor()
    }

    pub fn active_window_title(&mut self) -> Result<Option<String>, DiagnosableError> {
        if self.environment.is_some() {
            return Ok(None);
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        self.shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .active_window_title()
    }

    pub fn find_window_by_processes(
        &mut self,
        processes: &[String],
    ) -> Result<Option<String>, DiagnosableError> {
        if self.environment.is_some() {
            return Ok(None);
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        self.shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .find_window_by_processes(processes)
    }

    pub fn activate_window(&mut self, handle: &str) -> Result<bool, DiagnosableError> {
        if self.environment.is_some() {
            return Ok(false);
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        self.shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .activate_window(handle)
    }

    pub fn active_window_matches(&mut self, handle: &str) -> Result<bool, DiagnosableError> {
        if self.environment.is_some() {
            return Ok(false);
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        self.shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .active_window_matches(handle)
    }

    pub fn configure_input_provider(
        &mut self,
        provider: Option<&InputProviderConfig>,
        leader: Option<&MotionToken>,
    ) -> Result<(), DiagnosableError> {
        let Some(provider) = provider else {
            self.evdev_provider = None;
            self.uinput_session = None;
            return Ok(());
        };
        match provider.backend {
            InputProviderBackend::Evdev => {
                let devices = if provider.all_devices {
                    crate::evdev::discover_event_devices()?
                } else {
                    provider.devices.clone()
                };
                self.evdev_provider = Some(crate::evdev::EvdevObservationProvider::open(
                    devices,
                    provider.mode,
                    leader.cloned(),
                    provider.all_devices,
                )?);
            }
        }
        if provider.output == signal_auras_core::InputProviderOutput::Uinput {
            self.uinput_session = Some(crate::uinput::UinputOutputSession::open()?);
        } else {
            self.uinput_session = None;
        }
        Ok(())
    }

    // This is the side-effect boundary for live Wayland session probing. It
    // intentionally fails closed until a compositor-specific provider is wired
    // behind the adapter contracts.
    pub fn probe_capabilities(&self, required: &CapabilitySet) -> CapabilityReport {
        let report = match &self.environment {
            Some(environment) => crate::capability::kde_capability_report(required, environment),
            None => environment_probe(required),
        };
        if self.evdev_provider.is_some()
            && required.contains(CapabilityKind::CompositePointerObservation)
        {
            let report = report.with_status(CapabilityStatus::available(
                CapabilityKind::CompositePointerObservation,
                "evdev",
            ));
            let report = if self
                .evdev_provider
                .as_ref()
                .is_some_and(crate::evdev::EvdevObservationProvider::is_grab_capable)
                && self.uinput_session.is_some()
                && required.contains(CapabilityKind::CompositePointerConsumption)
            {
                report.with_status(CapabilityStatus::available(
                    CapabilityKind::CompositePointerConsumption,
                    "evdev-armed-grab",
                ))
            } else {
                report
            };
            if self.uinput_session.is_some() && required.contains(CapabilityKind::SynthesizedInput)
            {
                report.with_status(CapabilityStatus::available(
                    CapabilityKind::SynthesizedInput,
                    "uinput",
                ))
            } else {
                report
            }
        } else {
            if self.uinput_session.is_some() && required.contains(CapabilityKind::SynthesizedInput)
            {
                report.with_status(CapabilityStatus::available(
                    CapabilityKind::SynthesizedInput,
                    "uinput",
                ))
            } else {
                report
            }
        }
    }

    pub fn overlay_provider_report(&self) -> OverlayProviderReport {
        let environment = self
            .environment
            .clone()
            .unwrap_or_else(crate::capability::KdeEnvironment::from_process_env);
        self.overlay_renderer.provider_report(&environment)
    }

    pub fn render_overlay_snapshot(
        &mut self,
        snapshot: OverlaySnapshot,
    ) -> Result<(), DiagnosableError> {
        let overlay_id = snapshot.overlay_id.clone();
        let placement = crate::overlay::overlay_window_placement(&snapshot);
        self.overlay_renderer.render_snapshot(snapshot)?;
        if self.environment.is_some() {
            return Ok(());
        }
        let Some(placement) = placement else {
            self.overlay_placements.remove(&overlay_id);
            self.overlay_placement_attempts.remove(&overlay_id);
            return Ok(());
        };
        if self.overlay_placements.get(&overlay_id) == Some(&placement) {
            return Ok(());
        }
        let attempts = self
            .overlay_placement_attempts
            .get(&overlay_id)
            .and_then(|(pending, attempts)| (pending == &placement).then_some(*attempts))
            .unwrap_or(0);
        if attempts >= 20 {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                format!(
                    "native overlay window '{}' was not found by KWin after QML startup",
                    placement.title
                ),
            )
            .with_source("kwin-scripting")
            .with_remediation(
                "verify the Qt qml runtime can create windows and KWin scripting can enumerate them",
            ));
        }
        if self.shortcut_bridge.is_none() {
            self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
        }
        let placed = self
            .shortcut_bridge
            .as_mut()
            .expect("shortcut bridge was initialized")
            .configure_overlay_window(&placement)?;
        if placed {
            tracing::debug!(
                event = "overlay_window_placed",
                overlay_id = %placement.overlay_id,
                title = %placement.title,
                x = placement.x,
                y = placement.y,
                w = placement.w,
                h = placement.h,
            );
            self.overlay_placements
                .insert(overlay_id.clone(), placement);
            self.overlay_placement_attempts.remove(&overlay_id);
        } else {
            tracing::debug!(
                event = "overlay_window_not_found",
                overlay_id = %placement.overlay_id,
                title = %placement.title,
                attempt = attempts + 1,
            );
            self.overlay_placement_attempts
                .insert(overlay_id, (placement, attempts + 1));
        }
        Ok(())
    }

    pub fn cleanup_overlays(&mut self) -> Result<CleanupReport, DiagnosableError> {
        let report = self.overlay_renderer.cleanup_all()?;
        self.overlay_placements.clear();
        self.overlay_placement_attempts.clear();
        Ok(report)
    }

    pub fn active_overlay_snapshot_for_test(&self, overlay_id: &str) -> Option<&OverlaySnapshot> {
        self.overlay_renderer.active_snapshot(overlay_id)
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        CleanupReport::all_succeeded(self.registrations.len())
    }

    pub fn close_screen_cast_session(&mut self) -> CleanupReport {
        self.screen_cast_session
            .as_mut()
            .map(crate::portal::PortalScreenCastSession::close)
            .unwrap_or_else(CleanupReport::empty)
    }

    pub fn callback_wake_fd(&self) -> Option<std::os::fd::RawFd> {
        self.shortcut_bridge
            .as_ref()
            .map(crate::kde_bridge::KwinShortcutBridge::callback_wake_fd)
    }

    pub fn drain_callback_wake_fd(&self) -> Result<bool, DiagnosableError> {
        self.shortcut_bridge
            .as_ref()
            .map_or(Ok(false), |bridge| bridge.drain_callback_wake_fd())
    }

    pub fn take_callback_dropped_count(&mut self) -> u64 {
        self.shortcut_bridge
            .as_mut()
            .map(crate::kde_bridge::KwinShortcutBridge::take_callback_dropped_count)
            .unwrap_or_default()
    }

    pub fn next_shortcut_event(&mut self) -> Option<crate::kde_bridge::ObservedShortcutEvent> {
        self.shortcut_bridge
            .as_mut()
            .and_then(crate::kde_bridge::KwinShortcutBridge::next_shortcut_event)
    }

    pub fn next_motion_input_event(
        &mut self,
    ) -> Result<Option<MotionInputEvent>, DiagnosableError> {
        let Some(provider) = &mut self.evdev_provider else {
            return Ok(None);
        };
        provider.next_motion_event()
    }

    pub fn wait_next_motion_input_event(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<crate::evdev::ObservedMotionInputEvent>, DiagnosableError> {
        let Some(provider) = &mut self.evdev_provider else {
            if !timeout.is_zero() {
                std::thread::sleep(timeout);
            }
            return Ok(None);
        };
        provider.wait_next_observed_motion_event(timeout)
    }

    pub fn wait_next_motion_input_or_runtime_fd(
        &mut self,
        timeout: std::time::Duration,
        runtime_fds: &[std::os::fd::RawFd],
    ) -> Result<crate::evdev::EvdevWaitOutcome, DiagnosableError> {
        let Some(provider) = &mut self.evdev_provider else {
            return wait_runtime_fd(timeout, runtime_fds).map(|fd| {
                fd.map_or(
                    crate::evdev::EvdevWaitOutcome::Timeout,
                    crate::evdev::EvdevWaitOutcome::RuntimeFd,
                )
            });
        };
        provider.wait_next_observed_motion_event_or_runtime_fd(timeout, runtime_fds)
    }

    pub fn wait_next_input_or_runtime_fd(
        &mut self,
        timeout: std::time::Duration,
        runtime_fds: &[std::os::fd::RawFd],
    ) -> Result<crate::evdev::EvdevInputWaitOutcome, DiagnosableError> {
        let Some(provider) = &mut self.evdev_provider else {
            return wait_runtime_fd(timeout, runtime_fds).map(|fd| {
                fd.map_or(
                    crate::evdev::EvdevInputWaitOutcome::Timeout,
                    crate::evdev::EvdevInputWaitOutcome::RuntimeFd,
                )
            });
        };
        provider.wait_next_observed_input_event_or_runtime_fd(timeout, runtime_fds)
    }

    pub fn next_input_event(
        &mut self,
    ) -> Result<Option<crate::evdev::ObservedInputEvent>, DiagnosableError> {
        let Some(provider) = &mut self.evdev_provider else {
            return Ok(None);
        };
        provider.next_observed_input_event()
    }

    pub fn arm_input_grab(&mut self) -> Result<(), DiagnosableError> {
        if self.uinput_session.is_none() {
            return Ok(());
        }
        if let Some(provider) = &mut self.evdev_provider {
            provider.arm_grab()?;
        }
        Ok(())
    }

    pub fn release_input_grab(&mut self) -> Result<(), DiagnosableError> {
        if let Some(provider) = &mut self.evdev_provider {
            provider.release_grab();
        }
        Ok(())
    }

    pub fn passthrough_raw_input(
        &mut self,
        raw: &crate::evdev::RawInputEvent,
    ) -> Result<(), DiagnosableError> {
        if let Some(session) = &mut self.uinput_session {
            session.passthrough_raw(raw)?;
        }
        Ok(())
    }

    pub fn input_provider_summary(&self) -> Option<String> {
        self.evdev_provider.as_ref().map(|provider| {
            format!(
                "backend=evdev devices={} active_devices={} grabbed={} output={}",
                provider.device_count(),
                provider.active_device_count(),
                provider.is_grabbed(),
                if self.uinput_session.is_some() {
                    "uinput"
                } else {
                    "portal"
                }
            )
        })
    }
}

impl Default for RealWaylandAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn wait_runtime_fd(
    timeout: std::time::Duration,
    runtime_fds: &[std::os::fd::RawFd],
) -> Result<Option<std::os::fd::RawFd>, DiagnosableError> {
    if runtime_fds.is_empty() {
        if !timeout.is_zero() {
            std::thread::sleep(timeout);
        }
        return Ok(None);
    }
    let mut pollfds = runtime_fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect::<Vec<_>>();
    let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // Safety: poll is called with a valid mutable slice of pollfd values and
    // the call does not outlive the runtime-owned descriptors.
    let result = unsafe {
        libc::poll(
            pollfds.as_mut_ptr(),
            pollfds.len() as libc::nfds_t,
            timeout_ms,
        )
    };
    if result < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
        return Ok(None);
    }
    if result < 0 {
        return Err(DiagnosableError::new(
            ErrorPhase::Trigger,
            format!(
                "cannot poll runtime fds: {}",
                std::io::Error::last_os_error()
            ),
        )
        .with_source("runtime-event-loop"));
    }
    Ok(pollfds
        .iter()
        .find(|pollfd| pollfd.revents & libc::POLLIN != 0)
        .map(|pollfd| pollfd.fd))
}

impl ActiveProcessProvider for RealWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(None)
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        if let Some(bridge) = &self.shortcut_bridge {
            return Ok(bridge.active_process_context());
        }
        Ok(ActiveProcessContext::unavailable(
            "active process metadata provider is unsupported",
        ))
    }
}

impl HotkeyRegistrar for RealWaylandAdapter {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        let required = CapabilitySet::for_bindings([&binding]);
        if let Some(error) = self
            .probe_capabilities(&required)
            .first_blocking_error(&required)
        {
            return Err(error);
        }
        if !binding.trigger.is_keyboard() {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "composite pointer registration provider is unsupported",
            )
            .with_capability(Capability::CompositePointerObservation));
        }
        let signal_auras_core::BindingTrigger::Keyboard(hotkey) = &binding.trigger else {
            unreachable!("composite triggers returned above")
        };
        if self.rejected_hotkeys.contains(hotkey.as_str()) {
            return Err(crate::diagnostics::reserved_shortcut(hotkey.as_str()));
        }
        let id = if self.environment.is_some() {
            RegistrationId::new(format!("kde-{}", hotkey.as_str()))
        } else {
            if self.shortcut_bridge.is_none() {
                self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
            }
            RegistrationId::new(
                self.shortcut_bridge
                    .as_mut()
                    .expect("shortcut bridge was initialized")
                    .register_shortcut(&binding)?,
            )
        };
        self.registrations.push(id.clone());
        Ok(id)
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.cleanup_overlays()?;
        if let Some(bridge) = &mut self.shortcut_bridge {
            let _ = bridge.unload()?;
        }
        self.shortcut_bridge = None;
        self.registrations.clear();
        Ok(())
    }
}

impl MacroExecutor for RealWaylandAdapter {
    fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "synthesized input provider is unsupported",
        )
        .with_capability(Capability::SynthesizedInput))
    }

    fn execute_input_request(
        &mut self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        let required = CapabilitySet::new([CapabilityKind::SynthesizedInput]);
        if let Some(error) = self
            .probe_capabilities(&required)
            .first_blocking_error(&required)
        {
            return Err(error);
        }
        if let Some(session) = &mut self.uinput_session {
            session.synthesize(&request)?;
            return Ok(InputEmission::Emitted);
        }
        if self.portal_session.is_none() {
            self.portal_session = Some(if self.environment.is_some() {
                crate::portal::PortalInputSession::open()
            } else {
                crate::portal::PortalInputSession::open_live()?
            });
        }
        self.portal_session.as_ref().unwrap().synthesize(request)
    }

    fn cancel_pending(&mut self) -> Result<(), DiagnosableError> {
        self.release_input_grab()?;
        if let Some(session) = &mut self.portal_session {
            let _ = session.close();
        }
        if let Some(session) = &mut self.screen_cast_session {
            let _ = session.close();
        }
        self.portal_session = None;
        self.screen_cast_session = None;
        self.uinput_session = None;
        Ok(())
    }
}

impl ScreenSampleProvider for RealWaylandAdapter {
    fn capture_screen_sample(&mut self) -> Result<ScreenSample, DiagnosableError> {
        if self.screen_cast_session.is_none() {
            tracing::trace!(
                event = "screen_read_capture",
                phase = "session_open",
                "opening xdg-desktop-portal ScreenCast session"
            );
            self.screen_cast_session = Some(crate::portal::PortalScreenCastSession::open_live()?);
            tracing::trace!(
                event = "screen_read_capture",
                phase = "session_ready",
                "xdg-desktop-portal ScreenCast session is ready"
            );
        }
        tracing::trace!(
            event = "screen_read_capture",
            phase = "frame_request",
            "requesting latest readable screen frame"
        );
        self.screen_cast_session
            .as_mut()
            .expect("screen cast session was initialized")
            .capture_latest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{KdeEnvironment, KdeServiceAvailability};
    use signal_auras_core::{
        OverlayLifecycleState, OverlayRect, RendererProviderId, VisualSnapshot,
    };

    #[test]
    fn cancel_pending_releases_current_run_input_sessions() {
        let mut adapter = RealWaylandAdapter::from_environment(KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: KdeServiceAvailability::available(),
        });
        adapter.portal_session = Some(crate::portal::PortalInputSession::open());

        adapter.cancel_pending().unwrap();

        assert!(adapter.portal_session.is_none());
        assert!(adapter.uinput_session.is_none());
    }

    #[test]
    fn real_adapter_reports_renders_and_cleans_up_native_overlay_snapshots() {
        let mut adapter = RealWaylandAdapter::from_environment(
            crate::overlay::available_overlay_environment_for_test(),
        );

        assert!(adapter
            .overlay_provider_report()
            .status(RendererProviderId::Native)
            .availability
            .allows_activation());

        adapter
            .render_overlay_snapshot(native_overlay_snapshot("poe2-status"))
            .unwrap();
        assert!(adapter
            .active_overlay_snapshot_for_test("poe2-status")
            .is_some());

        adapter.unregister_all().unwrap();
        assert!(adapter
            .active_overlay_snapshot_for_test("poe2-status")
            .is_none());
    }

    fn native_overlay_snapshot(id: &str) -> OverlaySnapshot {
        OverlaySnapshot {
            overlay_id: id.to_string(),
            provider: RendererProviderId::Native,
            lifecycle: OverlayLifecycleState::Active,
            visuals: vec![VisualSnapshot {
                visual_id: "heavy_stun".to_string(),
                rect: OverlayRect {
                    x: 20,
                    y: 30,
                    w: 240,
                    h: 18,
                },
                opacity: 0.7,
                fill: "#d8b84c".to_string(),
                background: "#101820".to_string(),
                label_visible: true,
                fill_fraction: 0.42,
                active: true,
                ready: false,
            }],
            diagnostic: None,
        }
    }
}
