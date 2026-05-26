use signal_auras_core::{ActiveProcessContext, ProcessName};

pub fn name_only_context(name: ProcessName) -> ActiveProcessContext {
    ActiveProcessContext::name_only(name)
}
