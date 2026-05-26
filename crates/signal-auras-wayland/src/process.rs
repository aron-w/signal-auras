use signal_auras_core::{ActiveProcessContext, CapabilityKind, CapabilitySet, ProcessName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KwinWindowSnapshot {
    pub visible_name: Option<ProcessName>,
    pub process_id: Option<u32>,
    pub app_id: Option<String>,
    pub window_class: Option<String>,
    pub privileged: bool,
}

impl KwinWindowSnapshot {
    pub fn focused_app(visible_name: ProcessName) -> Self {
        Self {
            visible_name: Some(visible_name),
            process_id: None,
            app_id: None,
            window_class: None,
            privileged: false,
        }
    }

    pub fn into_context(self) -> ActiveProcessContext {
        if self.privileged {
            return ActiveProcessContext::unavailable(
                "KDE focused surface is privileged or compositor-owned",
            );
        }
        let Some(visible_name) = self.visible_name else {
            return ActiveProcessContext::unavailable("KDE active-process metadata is unavailable");
        };
        let mut context = if self.process_id.is_some() || self.app_id.is_some() {
            ActiveProcessContext::exact(visible_name, self.process_id)
        } else {
            ActiveProcessContext::name_only(visible_name)
        };
        if let Some(app_id) = self.app_id {
            context = context.with_app_id(app_id);
        }
        if let Some(window_class) = self.window_class {
            context = context.with_window_class(window_class);
        }
        context
    }
}

pub fn name_only_context(name: ProcessName) -> ActiveProcessContext {
    ActiveProcessContext::name_only(name)
}

pub fn active_process_capability_set() -> CapabilitySet {
    CapabilitySet::new([CapabilityKind::ActiveProcessMetadata])
}
