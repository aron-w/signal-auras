use crate::{
    AdapterDiagnostic, BindingMode, Capability, CapabilityAvailability, CapabilityKind,
    CapabilityReport, CapabilitySet, CapabilityStatus, DiagnosableError, ErrorPhase, HeldCondition,
    InputProviderConfig, MacroAction, MotionToken, MotionTrigger, ScopeSelection,
    SynthesizedInputRequest,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControllerRegistrationKind {
    Hotkey,
    Motion,
    Press,
    Timer,
    Shutdown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CallbackOverloadPolicy {
    #[default]
    SkipWhilePending,
    DropNewest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerRegistration {
    pub kind: ControllerRegistrationKind,
    pub trigger: String,
    pub scope: ScopeSelection,
    pub mode: BindingMode,
    pub callback: String,
    pub required_capabilities: CapabilitySet,
    pub overload_policy: CallbackOverloadPolicy,
    pub requires_held: HeldCondition,
    pub loop_policy: Option<ControllerLoopPolicy>,
}

impl ControllerRegistration {
    pub fn new(
        kind: ControllerRegistrationKind,
        trigger: impl Into<String>,
        scope: ScopeSelection,
        mode: BindingMode,
        callback: impl Into<String>,
        required_capabilities: CapabilitySet,
    ) -> Result<Self, DiagnosableError> {
        let trigger = normalize_label(trigger.into(), "controller registration trigger")?;
        let callback = normalize_label(callback.into(), "controller callback name")?;
        Ok(Self {
            kind,
            trigger,
            scope,
            mode,
            callback,
            required_capabilities,
            overload_policy: CallbackOverloadPolicy::default(),
            requires_held: HeldCondition::new(Vec::new())?,
            loop_policy: None,
        })
    }

    pub fn with_overload_policy(mut self, overload_policy: CallbackOverloadPolicy) -> Self {
        self.overload_policy = overload_policy;
        self
    }

    pub fn with_requires_held(mut self, requires_held: HeldCondition) -> Self {
        self.requires_held = requires_held;
        self
    }

    pub fn with_loop_policy(mut self, loop_policy: Option<ControllerLoopPolicy>) -> Self {
        self.loop_policy = loop_policy;
        self
    }

    pub fn with_callback(mut self, callback: impl Into<String>) -> Result<Self, DiagnosableError> {
        self.callback = normalize_label(callback.into(), "controller callback name")?;
        Ok(self)
    }

    pub fn label(&self) -> String {
        format!("{:?}:{}", self.kind, self.trigger)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerLoopPolicy {
    pub while_held: MotionTrigger,
    pub before_callback: Option<String>,
    pub repeat_every_ms: u64,
    pub repeat_callback: String,
    pub after_callback: Option<String>,
}

impl ControllerLoopPolicy {
    pub fn new(
        while_held: MotionTrigger,
        before_callback: Option<impl Into<String>>,
        repeat_every_ms: u64,
        repeat_callback: impl Into<String>,
        after_callback: Option<impl Into<String>>,
    ) -> Result<Self, DiagnosableError> {
        if repeat_every_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "controller loop repeat every_ms must be a positive integer",
            ));
        }
        Ok(Self {
            while_held,
            before_callback: before_callback
                .map(|callback| normalize_label(callback.into(), "controller loop before callback"))
                .transpose()?,
            repeat_every_ms,
            repeat_callback: normalize_label(
                repeat_callback.into(),
                "controller loop repeat callback",
            )?,
            after_callback: after_callback
                .map(|callback| normalize_label(callback.into(), "controller loop after callback"))
                .transpose()?,
        })
    }
}

fn normalize_label(value: String, field: &'static str) -> Result<String, DiagnosableError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} cannot be empty"),
        ));
    }
    Ok(value.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerRegistrationSet {
    registrations: Vec<ControllerRegistration>,
    required_capabilities: CapabilitySet,
}

impl ControllerRegistrationSet {
    pub fn new(
        registrations: impl IntoIterator<Item = ControllerRegistration>,
    ) -> Result<Self, DiagnosableError> {
        let registrations = registrations.into_iter().collect::<Vec<_>>();
        if registrations.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "controller must register at least one handler",
            ));
        }

        let mut seen = BTreeSet::new();
        let mut required = Vec::new();
        for registration in &registrations {
            let key = (
                registration.kind,
                registration.scope_label(),
                registration.trigger.clone(),
            );
            if !seen.insert(key) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "duplicate controller trigger after normalization: '{}'",
                        registration.trigger
                    ),
                ));
            }
            required.extend(registration.required_capabilities.iter());
        }
        reject_prefix_overlapping_motions(&registrations)?;
        Ok(Self {
            registrations,
            required_capabilities: CapabilitySet::new(required),
        })
    }

    pub fn registrations(&self) -> &[ControllerRegistration] {
        &self.registrations
    }

    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    pub fn validate_capabilities(&self, report: &CapabilityReport) -> Result<(), DiagnosableError> {
        report
            .first_blocking_error(&self.required_capabilities)
            .map_or(Ok(()), Err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerCallback {
    pub name: String,
    pub actions: Vec<MacroAction>,
}

impl ControllerCallback {
    pub fn new(
        name: impl Into<String>,
        actions: impl IntoIterator<Item = MacroAction>,
    ) -> Result<Self, DiagnosableError> {
        let name = normalize_label(name.into(), "controller callback name")?;
        Ok(Self {
            name,
            actions: actions.into_iter().collect(),
        })
    }

    pub fn required_capabilities(&self) -> CapabilitySet {
        if self.actions.is_empty() {
            CapabilitySet::default()
        } else {
            CapabilitySet::new([CapabilityKind::SynthesizedInput])
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerProgram {
    registrations: ControllerRegistrationSet,
    callbacks: BTreeMap<String, ControllerCallback>,
    required_capabilities: CapabilitySet,
    pub input_provider: Option<InputProviderConfig>,
    pub leader: Option<MotionToken>,
}

impl ControllerProgram {
    pub fn new(
        registrations: ControllerRegistrationSet,
        callbacks: impl IntoIterator<Item = ControllerCallback>,
    ) -> Result<Self, DiagnosableError> {
        let callbacks = callbacks
            .into_iter()
            .map(|callback| (callback.name.clone(), callback))
            .collect::<BTreeMap<_, _>>();
        for registration in registrations.registrations() {
            if !callbacks.contains_key(&registration.callback) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "controller callback '{}' is registered but not defined",
                        registration.callback
                    ),
                ));
            }
            if let Some(loop_policy) = &registration.loop_policy {
                for callback in [
                    loop_policy.before_callback.as_deref(),
                    Some(loop_policy.repeat_callback.as_str()),
                    loop_policy.after_callback.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    if !callbacks.contains_key(callback) {
                        return Err(DiagnosableError::new(
                            ErrorPhase::ScriptValidation,
                            format!(
                                "controller loop callback '{callback}' is registered but not defined"
                            ),
                        ));
                    }
                }
            }
        }
        let mut required = registrations
            .required_capabilities()
            .iter()
            .collect::<Vec<_>>();
        for callback in callbacks.values() {
            required.extend(callback.required_capabilities().iter());
        }
        Ok(Self {
            registrations,
            callbacks,
            required_capabilities: CapabilitySet::new(required),
            input_provider: None,
            leader: None,
        })
    }

    pub fn with_runtime_options(
        mut self,
        input_provider: Option<InputProviderConfig>,
        leader: Option<MotionToken>,
    ) -> Self {
        self.input_provider = input_provider;
        self.leader = leader;
        self
    }

    pub fn registrations(&self) -> &ControllerRegistrationSet {
        &self.registrations
    }

    pub fn callbacks(&self) -> impl Iterator<Item = &ControllerCallback> {
        self.callbacks.values()
    }

    pub fn callback(&self, name: &str) -> Option<&ControllerCallback> {
        self.callbacks.get(name)
    }

    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    pub fn validate_capabilities(&self, report: &CapabilityReport) -> Result<(), DiagnosableError> {
        report
            .first_blocking_error(&self.required_capabilities)
            .map_or(Ok(()), Err)
    }
}

impl ControllerRegistration {
    fn scope_label(&self) -> String {
        match &self.scope {
            ScopeSelection::ExplicitGlobal => "global".to_string(),
            ScopeSelection::ProcessList { processes } => processes
                .iter()
                .map(|process| process.as_str().to_string())
                .collect::<Vec<_>>()
                .join(","),
        }
    }
}

fn reject_prefix_overlapping_motions(
    registrations: &[ControllerRegistration],
) -> Result<(), DiagnosableError> {
    let motions = registrations
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Motion)
        .collect::<Vec<_>>();
    for (index, left) in motions.iter().enumerate() {
        for right in motions.iter().skip(index + 1) {
            let left_tokens = left.trigger.split(' ').collect::<Vec<_>>();
            let right_tokens = right.trigger.split(' ').collect::<Vec<_>>();
            if left.scope_label() == right.scope_label()
                && ((left_tokens.len() < right_tokens.len()
                    && right_tokens.starts_with(&left_tokens))
                    || (right_tokens.len() < left_tokens.len()
                        && left_tokens.starts_with(&right_tokens)))
            {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "prefix-overlapping controller motion triggers are ambiguous: '{}' and '{}'",
                        left.trigger, right.trigger
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackDisposition {
    Accepted,
    Skipped,
    Denied,
    Dropped,
    Completed,
    Slow,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaCallbackTask {
    pub registration_label: String,
    pub callback: String,
    pub accepted_at: Instant,
    pub budget: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackScheduleResult {
    pub disposition: CallbackDisposition,
    pub diagnostic: Option<AdapterDiagnostic>,
}

impl CallbackScheduleResult {
    fn new(disposition: CallbackDisposition) -> Self {
        Self {
            disposition,
            diagnostic: None,
        }
    }

    fn with_diagnostic(disposition: CallbackDisposition, diagnostic: AdapterDiagnostic) -> Self {
        Self {
            disposition,
            diagnostic: Some(diagnostic),
        }
    }
}

#[derive(Debug)]
pub struct LuaCallbackScheduler {
    max_pending: usize,
    default_budget: Duration,
    pending: VecDeque<LuaCallbackTask>,
    active_or_pending: BTreeSet<String>,
}

impl LuaCallbackScheduler {
    pub fn new(max_pending: usize, default_budget: Duration) -> Result<Self, DiagnosableError> {
        if max_pending == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "callback scheduler capacity must be greater than zero",
            ));
        }
        if default_budget.is_zero() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "callback budget must be greater than zero",
            ));
        }
        Ok(Self {
            max_pending,
            default_budget,
            pending: VecDeque::new(),
            active_or_pending: BTreeSet::new(),
        })
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn schedule(
        &mut self,
        registration: &ControllerRegistration,
        capabilities: &CapabilityReport,
        accepted_at: Instant,
    ) -> CallbackScheduleResult {
        if let Some(error) = capabilities.first_blocking_error(&registration.required_capabilities)
        {
            return CallbackScheduleResult::with_diagnostic(
                CallbackDisposition::Denied,
                AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, error.message).with_capability(
                    error
                        .capability
                        .map(capability_to_kind)
                        .unwrap_or(CapabilityKind::SynthesizedInput),
                ),
            );
        }

        let label = registration.label();
        if self.active_or_pending.contains(&label) {
            return CallbackScheduleResult::new(match registration.overload_policy {
                CallbackOverloadPolicy::SkipWhilePending => CallbackDisposition::Skipped,
                CallbackOverloadPolicy::DropNewest => CallbackDisposition::Dropped,
            });
        }
        if self.pending.len() >= self.max_pending {
            return CallbackScheduleResult::new(CallbackDisposition::Dropped);
        }
        self.active_or_pending.insert(label.clone());
        self.pending.push_back(LuaCallbackTask {
            registration_label: label,
            callback: registration.callback.clone(),
            accepted_at,
            budget: self.default_budget,
        });
        CallbackScheduleResult::new(CallbackDisposition::Accepted)
    }

    pub fn pop_next(&mut self) -> Option<LuaCallbackTask> {
        self.pending.pop_front()
    }

    pub fn finish(&mut self, task: LuaCallbackTask, elapsed: Duration) -> CallbackDisposition {
        self.active_or_pending.remove(&task.registration_label);
        if elapsed > task.budget {
            CallbackDisposition::Slow
        } else {
            CallbackDisposition::Completed
        }
    }

    pub fn cancel_all(&mut self) -> usize {
        let cancelled = self.pending.len();
        self.pending.clear();
        self.active_or_pending.clear();
        cancelled
    }
}

fn capability_to_kind(capability: Capability) -> CapabilityKind {
    match capability {
        Capability::GlobalShortcut => CapabilityKind::GlobalShortcut,
        Capability::CompositePointerObservation => CapabilityKind::CompositePointerObservation,
        Capability::CompositePointerConsumption => CapabilityKind::CompositePointerConsumption,
        Capability::ActiveProcess => CapabilityKind::ActiveProcessMetadata,
        Capability::ActiveWindowMetadata => CapabilityKind::ActiveWindowMetadata,
        Capability::WindowActivation => CapabilityKind::WindowActivation,
        Capability::SynthesizedInput => CapabilityKind::SynthesizedInput,
        Capability::Timer => CapabilityKind::Timer,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustOperationBatch {
    max_pending: usize,
    requests: Vec<SynthesizedInputRequest>,
    next_sequence: usize,
}

impl RustOperationBatch {
    pub fn new(max_pending: usize) -> Result<Self, DiagnosableError> {
        if max_pending == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "output batch capacity must be greater than zero",
            ));
        }
        Ok(Self {
            max_pending,
            requests: Vec::new(),
            next_sequence: 0,
        })
    }

    pub fn enqueue_input(
        &mut self,
        action: MacroAction,
        capabilities: &CapabilityReport,
    ) -> Result<(), DiagnosableError> {
        let required = CapabilitySet::new([CapabilityKind::SynthesizedInput]);
        if let Some(error) = capabilities.first_blocking_error(&required) {
            return Err(error);
        }
        if self.requests.len() >= self.max_pending {
            return Err(
                DiagnosableError::new(ErrorPhase::MacroExecution, "output queue is full")
                    .with_capability(Capability::SynthesizedInput),
            );
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.requests
            .push(SynthesizedInputRequest::new(action, sequence));
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<SynthesizedInputRequest> {
        std::mem::take(&mut self.requests)
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

pub fn queue_controller_callback_outputs(
    callback: &ControllerCallback,
    capabilities: &CapabilityReport,
    batch: &mut RustOperationBatch,
) -> Result<(), DiagnosableError> {
    for action in &callback.actions {
        batch.enqueue_input(action.clone(), capabilities)?;
    }
    Ok(())
}

pub fn available_capability_report(required: &CapabilitySet, source: &str) -> CapabilityReport {
    CapabilityReport::from_statuses(
        required
            .iter()
            .map(|kind| CapabilityStatus::available(kind, source)),
    )
}

pub fn denied_capability_report(kind: CapabilityKind, message: &str) -> CapabilityReport {
    CapabilityReport::from_statuses([CapabilityStatus::unavailable(
        kind,
        CapabilityAvailability::Denied,
        AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, message).with_capability(kind),
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProcessName, ScopeSelection};

    fn hotkey_registration(trigger: &str) -> ControllerRegistration {
        ControllerRegistration::new(
            ControllerRegistrationKind::Hotkey,
            trigger,
            ScopeSelection::ExplicitGlobal,
            BindingMode::Consume,
            "callback",
            CapabilitySet::new([CapabilityKind::GlobalShortcut]),
        )
        .unwrap()
    }

    #[test]
    fn registration_set_rejects_duplicate_triggers_after_normalization() {
        let error = ControllerRegistrationSet::new([
            hotkey_registration("F5"),
            hotkey_registration("  F5  "),
        ])
        .unwrap_err();

        assert!(error.message.contains("duplicate controller trigger"));
    }

    #[test]
    fn registration_set_rejects_prefix_overlapping_motions() {
        let first = ControllerRegistration::new(
            ControllerRegistrationKind::Motion,
            "<Leader> x",
            ScopeSelection::ExplicitGlobal,
            BindingMode::Consume,
            "short",
            CapabilitySet::new([CapabilityKind::CompositePointerObservation]),
        )
        .unwrap();
        let second = ControllerRegistration::new(
            ControllerRegistrationKind::Motion,
            "<Leader> x x",
            ScopeSelection::ExplicitGlobal,
            BindingMode::Consume,
            "long",
            CapabilitySet::new([CapabilityKind::CompositePointerObservation]),
        )
        .unwrap();

        let error = ControllerRegistrationSet::new([first, second]).unwrap_err();

        assert!(error.message.contains("prefix-overlapping"));
    }

    #[test]
    fn registration_set_collects_capability_requirements() {
        let scoped = ControllerRegistration::new(
            ControllerRegistrationKind::Hotkey,
            "F6",
            ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap()]).unwrap(),
            BindingMode::Consume,
            "callback",
            CapabilitySet::new([
                CapabilityKind::GlobalShortcut,
                CapabilityKind::ActiveProcessMetadata,
            ]),
        )
        .unwrap();
        let set = ControllerRegistrationSet::new([hotkey_registration("F5"), scoped]).unwrap();

        assert!(set
            .required_capabilities()
            .contains(CapabilityKind::GlobalShortcut));
        assert!(set
            .required_capabilities()
            .contains(CapabilityKind::ActiveProcessMetadata));
    }

    #[test]
    fn callback_scheduler_bounds_pending_work_and_skips_same_trigger() {
        let registration = hotkey_registration("F5");
        let report = available_capability_report(&registration.required_capabilities, "test");
        let mut scheduler = LuaCallbackScheduler::new(2, Duration::from_millis(10)).unwrap();
        let now = Instant::now();

        assert_eq!(
            scheduler.schedule(&registration, &report, now).disposition,
            CallbackDisposition::Accepted
        );
        assert_eq!(
            scheduler.schedule(&registration, &report, now).disposition,
            CallbackDisposition::Skipped
        );
        let task = scheduler.pop_next().unwrap();
        assert_eq!(
            scheduler.finish(task, Duration::from_millis(11)),
            CallbackDisposition::Slow
        );
    }

    #[test]
    fn callback_scheduler_denies_unavailable_capabilities() {
        let registration = hotkey_registration("F5");
        let report = denied_capability_report(CapabilityKind::GlobalShortcut, "shortcut denied");
        let mut scheduler = LuaCallbackScheduler::new(2, Duration::from_millis(10)).unwrap();

        let result = scheduler.schedule(&registration, &report, Instant::now());

        assert_eq!(result.disposition, CallbackDisposition::Denied);
        assert!(result
            .diagnostic
            .unwrap()
            .message
            .contains("shortcut denied"));
    }

    #[test]
    fn output_batch_preserves_order_and_fails_closed_when_denied() {
        let required = CapabilitySet::new([CapabilityKind::SynthesizedInput]);
        let available = available_capability_report(&required, "test");
        let denied = denied_capability_report(CapabilityKind::SynthesizedInput, "input denied");
        let mut batch = RustOperationBatch::new(4).unwrap();

        batch
            .enqueue_input(MacroAction::text("hello").unwrap(), &available)
            .unwrap();
        batch
            .enqueue_input(MacroAction::key("Enter").unwrap(), &available)
            .unwrap();

        let requests = batch.drain();
        assert_eq!(requests[0].sequence, 0);
        assert_eq!(requests[1].sequence, 1);
        assert!(batch
            .enqueue_input(MacroAction::key("A").unwrap(), &denied)
            .is_err());
    }

    #[test]
    fn controller_program_requires_registered_callbacks_to_be_defined() {
        let registrations = ControllerRegistrationSet::new([hotkey_registration("F5")]).unwrap();

        let error = ControllerProgram::new(registrations, []).unwrap_err();

        assert!(error.message.contains("registered but not defined"));
    }

    #[test]
    fn controller_program_collects_callback_output_capabilities() {
        let registrations = ControllerRegistrationSet::new([hotkey_registration("F5")]).unwrap();
        let callback = ControllerCallback::new(
            "callback",
            [
                MacroAction::text("/hideout").unwrap(),
                MacroAction::key("Enter").unwrap(),
            ],
        )
        .unwrap();

        let program = ControllerProgram::new(registrations, [callback]).unwrap();

        assert!(program
            .required_capabilities()
            .contains(CapabilityKind::SynthesizedInput));
    }

    #[test]
    fn controller_callback_output_queue_fails_closed_when_capability_denied() {
        let callback =
            ControllerCallback::new("callback", [MacroAction::key("A").unwrap()]).unwrap();
        let denied = denied_capability_report(CapabilityKind::SynthesizedInput, "input denied");
        let mut batch = RustOperationBatch::new(4).unwrap();

        let error = queue_controller_callback_outputs(&callback, &denied, &mut batch).unwrap_err();

        assert_eq!(error.capability, Some(Capability::SynthesizedInput));
        assert!(batch.is_empty());
    }
}
