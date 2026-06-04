use signal_auras_core::{
    CleanupReport, DiagnosableError, ErrorPhase, OverlayDiagnosticReason, OverlayLifecycleState,
    OverlayProviderReport, OverlayProviderStatus, OverlaySnapshot, RendererProviderId,
};
use std::collections::BTreeMap;

use crate::capability::KdeEnvironment;
#[cfg(test)]
use crate::capability::KdeServiceAvailability;

pub trait OverlayRendererAdapter {
    fn provider(&self) -> RendererProviderId;
    fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError>;
    fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError>;
    fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError>;

    fn render_snapshot(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != self.provider() {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                format!(
                    "overlay renderer '{}' cannot render provider '{}' snapshots",
                    self.provider().as_str(),
                    snapshot.provider.as_str()
                ),
            ));
        }
        if snapshot.is_active() {
            return self.mount_or_update(snapshot);
        }
        let reason = snapshot
            .diagnostic
            .as_ref()
            .map(|diagnostic| diagnostic.reason)
            .unwrap_or(OverlayDiagnosticReason::ProviderUnavailable);
        match snapshot.lifecycle {
            OverlayLifecycleState::CleanedUp => self.cleanup(snapshot.overlay_id),
            _ => self.hide(snapshot.overlay_id, reason),
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryOverlayRenderer {
    mounted: Vec<OverlaySnapshot>,
    active: BTreeMap<String, OverlaySnapshot>,
    hidden: Vec<String>,
    cleaned_up: Vec<String>,
}

impl InMemoryOverlayRenderer {
    pub fn mounted(&self) -> &[OverlaySnapshot] {
        &self.mounted
    }

    pub fn hidden(&self) -> &[String] {
        &self.hidden
    }

    pub fn cleaned_up(&self) -> &[String] {
        &self.cleaned_up
    }

    pub fn active_snapshot(&self, overlay_id: &str) -> Option<&OverlaySnapshot> {
        self.active.get(overlay_id)
    }

    pub fn cleanup_all(&mut self) -> Result<CleanupReport, DiagnosableError> {
        let overlay_ids = self.active.keys().cloned().collect::<Vec<_>>();
        for overlay_id in &overlay_ids {
            self.cleanup(overlay_id.clone())?;
        }
        Ok(CleanupReport::all_succeeded(overlay_ids.len()))
    }

    pub fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != RendererProviderId::Native {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "in-memory overlay renderer supports only native provider snapshots",
            ));
        }
        self.active
            .insert(snapshot.overlay_id.clone(), snapshot.clone());
        self.mounted.push(snapshot);
        Ok(())
    }

    pub fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        _reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        self.hidden.push(overlay_id);
        Ok(())
    }

    pub fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        self.cleaned_up.push(overlay_id);
        Ok(())
    }

    pub fn last_lifecycle(&self) -> Option<OverlayLifecycleState> {
        self.mounted.last().map(|snapshot| snapshot.lifecycle)
    }
}

impl OverlayRendererAdapter for InMemoryOverlayRenderer {
    fn provider(&self) -> RendererProviderId {
        RendererProviderId::Native
    }

    fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        InMemoryOverlayRenderer::mount_or_update(self, snapshot)
    }

    fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        InMemoryOverlayRenderer::hide(self, overlay_id, reason)
    }

    fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        InMemoryOverlayRenderer::cleanup(self, overlay_id)
    }
}

pub type NativeOverlayRenderer = InMemoryOverlayRenderer;

pub fn provider_report_for_environment(environment: &KdeEnvironment) -> OverlayProviderReport {
    let native_status = if native_overlay_environment_available(environment) {
        OverlayProviderStatus::available(RendererProviderId::Native)
    } else {
        OverlayProviderStatus::unavailable(
            RendererProviderId::Native,
            "native overlay provider requires a KDE Plasma Wayland session with KWin scripting",
        )
    };
    OverlayProviderReport::from_statuses([
        native_status,
        OverlayProviderStatus::unavailable(
            RendererProviderId::WebView,
            "WebView overlay provider is declared for future adapter support",
        ),
        OverlayProviderStatus::unavailable(
            RendererProviderId::TauriWindow,
            "Tauri window overlay provider is declared for future adapter support",
        ),
        OverlayProviderStatus::unavailable(
            RendererProviderId::ToolWindow,
            "tool window overlay provider is declared for future adapter support",
        ),
    ])
}

fn native_overlay_environment_available(environment: &KdeEnvironment) -> bool {
    environment
        .wayland_display
        .as_deref()
        .is_some_and(not_empty)
        && environment
            .session_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        && environment
            .current_desktop
            .as_deref()
            .is_some_and(|desktop| {
                desktop
                    .split(':')
                    .any(|part| part.eq_ignore_ascii_case("KDE"))
            })
        && environment.services.kwin
}

fn not_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

#[cfg(test)]
pub(crate) fn available_overlay_environment_for_test() -> KdeEnvironment {
    KdeEnvironment {
        wayland_display: Some("wayland-0".to_string()),
        session_type: Some("wayland".to_string()),
        current_desktop: Some("KDE".to_string()),
        services: KdeServiceAvailability {
            kwin: true,
            ..KdeServiceAvailability::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{OverlayDiagnostic, OverlayRect, VisualSnapshot};

    #[test]
    fn provider_report_marks_native_available_only_for_kde_wayland_kwin() {
        let available = provider_report_for_environment(&available_overlay_environment_for_test());
        assert!(available
            .status(RendererProviderId::Native)
            .availability
            .allows_activation());
        assert!(!available
            .status(RendererProviderId::WebView)
            .availability
            .allows_activation());

        let unavailable = provider_report_for_environment(&KdeEnvironment {
            wayland_display: Some("wayland-0".to_string()),
            session_type: Some("wayland".to_string()),
            current_desktop: Some("GNOME".to_string()),
            services: KdeServiceAvailability {
                kwin: true,
                ..KdeServiceAvailability::default()
            },
        });
        assert!(!unavailable
            .status(RendererProviderId::Native)
            .availability
            .allows_activation());
    }

    #[test]
    fn native_renderer_updates_hides_and_cleans_up_sanitized_snapshots() {
        let mut renderer = NativeOverlayRenderer::default();
        let active = active_snapshot("poe2-bars");

        renderer.render_snapshot(active.clone()).unwrap();
        assert_eq!(renderer.mounted(), &[active.clone()]);
        assert_eq!(
            renderer.active_snapshot("poe2-bars").unwrap().visuals[0].fill,
            "#6ee7b7"
        );

        renderer
            .render_snapshot(OverlaySnapshot {
                lifecycle: OverlayLifecycleState::Inactive,
                diagnostic: Some(OverlayDiagnostic {
                    overlay_id: "poe2-bars".to_string(),
                    provider: RendererProviderId::Native,
                    lifecycle: OverlayLifecycleState::Inactive,
                    reason: OverlayDiagnosticReason::FocusInactive,
                    tracker_id: None,
                    field: None,
                    message: "focus inactive".to_string(),
                }),
                ..active.clone()
            })
            .unwrap();
        assert!(renderer.active_snapshot("poe2-bars").is_none());
        assert_eq!(renderer.hidden(), &["poe2-bars".to_string()]);

        renderer.render_snapshot(active).unwrap();
        let report = renderer.cleanup_all().unwrap();
        assert_eq!(report.attempted, 1);
        assert_eq!(renderer.cleaned_up(), &["poe2-bars".to_string()]);
    }

    #[test]
    fn native_renderer_rejects_future_provider_snapshots() {
        let mut renderer = NativeOverlayRenderer::default();
        let mut snapshot = active_snapshot("tool");
        snapshot.provider = RendererProviderId::WebView;

        let error = renderer.render_snapshot(snapshot).unwrap_err();
        assert!(error
            .message
            .contains("cannot render provider 'webview' snapshots"));
    }

    fn active_snapshot(id: &str) -> OverlaySnapshot {
        OverlaySnapshot {
            overlay_id: id.to_string(),
            provider: RendererProviderId::Native,
            lifecycle: OverlayLifecycleState::Active,
            visuals: vec![VisualSnapshot {
                visual_id: "heavy-stun".to_string(),
                rect: OverlayRect {
                    x: 10,
                    y: 20,
                    w: 160,
                    h: 12,
                },
                opacity: 0.65,
                fill: "#6ee7b7".to_string(),
                background: "#111827".to_string(),
                label_visible: false,
                fill_fraction: 0.5,
                active: true,
                ready: false,
            }],
            diagnostic: None,
        }
    }
}
