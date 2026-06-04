use signal_auras_core::{
    DiagnosableError, ErrorPhase, OverlayDiagnosticReason, OverlayLifecycleState, OverlaySnapshot,
    RendererProviderId,
};

#[derive(Debug, Default)]
pub struct InMemoryOverlayRenderer {
    pub mounted: Vec<OverlaySnapshot>,
    pub hidden: Vec<String>,
    pub cleaned_up: Vec<String>,
}

impl InMemoryOverlayRenderer {
    pub fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != RendererProviderId::Native {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "in-memory overlay renderer supports only native provider snapshots",
            ));
        }
        self.mounted.push(snapshot);
        Ok(())
    }

    pub fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        _reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        self.hidden.push(overlay_id.into());
        Ok(())
    }

    pub fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        self.cleaned_up.push(overlay_id.into());
        Ok(())
    }

    pub fn last_lifecycle(&self) -> Option<OverlayLifecycleState> {
        self.mounted.last().map(|snapshot| snapshot.lifecycle)
    }
}
