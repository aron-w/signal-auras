use signal_auras_core::{
    ActiveProcessContext, CleanupReport, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyId,
    ProcessName,
};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    os::fd::RawFd,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

const CALLBACK_QUEUE_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct ObservedShortcutEvent {
    pub hotkey: HotkeyId,
    pub received_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum KdeBridgeLifecycle {
    #[default]
    NotInstalled,
    Installing,
    Active,
    Unloading,
    Unloaded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KdeBridgeState {
    lifecycle: KdeBridgeLifecycle,
    registered_handles: usize,
    events: VecDeque<HotkeyId>,
}

impl KdeBridgeState {
    pub fn active_for_test(registered_handles: usize) -> Self {
        Self {
            lifecycle: KdeBridgeLifecycle::Active,
            registered_handles,
            events: VecDeque::new(),
        }
    }

    pub fn lifecycle(&self) -> &KdeBridgeLifecycle {
        &self.lifecycle
    }

    pub fn unload(&mut self) -> Result<CleanupReport, signal_auras_core::DiagnosableError> {
        if matches!(self.lifecycle, KdeBridgeLifecycle::Unloaded) {
            return Ok(CleanupReport::empty());
        }
        let report = self.cleanup_report();
        self.registered_handles = 0;
        self.events.clear();
        self.lifecycle = KdeBridgeLifecycle::Unloaded;
        Ok(report)
    }

    pub fn push_shortcut_event(&mut self, hotkey: HotkeyId) -> Result<(), DiagnosableError> {
        if !matches!(self.lifecycle, KdeBridgeLifecycle::Active) {
            return Err(DiagnosableError::new(
                ErrorPhase::Trigger,
                "KDE bridge is not active",
            ));
        }
        self.events.push_back(hotkey);
        Ok(())
    }

    pub fn next_shortcut_event(&mut self) -> Result<Option<HotkeyId>, DiagnosableError> {
        if !matches!(
            self.lifecycle,
            KdeBridgeLifecycle::Active | KdeBridgeLifecycle::Unloaded
        ) {
            return Err(DiagnosableError::new(
                ErrorPhase::Trigger,
                "KDE bridge cannot deliver events in its current state",
            ));
        }
        Ok(self.events.pop_front())
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        CleanupReport::all_succeeded(self.registered_handles)
    }
}

#[derive(Debug)]
pub struct KwinShortcutBridge {
    connection: zbus::blocking::Connection,
    queue: Arc<Mutex<KwinCallbackQueue>>,
    callback_wake_fd: crate::event_loop::RuntimeWakeFd,
    actions: BTreeMap<String, HotkeyId>,
    active_process: ActiveProcessContext,
    scripts: Vec<KwinScriptHandle>,
    active_process_monitor: Option<KwinScriptHandle>,
    next_index: usize,
    callback_bus_name: String,
    callback_object_path: String,
}

#[derive(Debug, Clone)]
struct KwinScriptHandle {
    action_name: String,
    script_id: String,
    script_path: PathBuf,
}

impl KwinShortcutBridge {
    pub fn connect() -> Result<Self, DiagnosableError> {
        let connection = zbus::blocking::Connection::session().map_err(bridge_error)?;
        let callback_bus_name = format!("org.signalAuras.Runner{}", std::process::id());
        let callback_object_path = "/org/signalAuras/Runner".to_string();
        let callback_wake_fd = crate::event_loop::RuntimeWakeFd::new()?;
        let wake_sender = callback_wake_fd.sender()?;
        let queue = Arc::new(Mutex::new(KwinCallbackQueue::new(CALLBACK_QUEUE_LIMIT)));
        spawn_kwin_callback_listener(
            &callback_bus_name,
            &callback_object_path,
            Arc::clone(&queue),
            wake_sender,
        )?;
        Ok(Self {
            connection,
            queue,
            callback_wake_fd,
            actions: BTreeMap::new(),
            active_process: ActiveProcessContext::unavailable(
                "KDE active-process metadata has not been received yet",
            ),
            scripts: Vec::new(),
            active_process_monitor: None,
            next_index: 0,
            callback_bus_name,
            callback_object_path,
        })
    }

    pub fn register_shortcut(
        &mut self,
        binding: &HotkeyBinding,
    ) -> Result<String, DiagnosableError> {
        let hotkey = binding.keyboard_hotkey().ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::Registration,
                "KWin shortcut bridge only accepts keyboard triggers",
            )
        })?;
        self.next_index += 1;
        let action_name = format!("SignalAuras_{}_{}", std::process::id(), self.next_index);
        let script_id = format!("signal-auras-{}-{}", std::process::id(), self.next_index);
        let script_path = std::env::temp_dir().join(format!("{script_id}.js"));
        let script = kwin_shortcut_script(
            &action_name,
            hotkey.as_str(),
            &format!("Signal Auras {}", hotkey.as_str()),
            &self.callback_bus_name,
            &self.callback_object_path,
        );
        fs::write(&script_path, script).map_err(bridge_error)?;

        let proxy = kwin_scripting_proxy(&self.connection)?;
        let loaded_id: i32 = proxy
            .call(
                "loadScript",
                &(script_path.to_string_lossy().as_ref(), script_id.as_str()),
            )
            .map_err(bridge_error)?;
        if loaded_id < 0 {
            let _ = fs::remove_file(&script_path);
            return Err(bridge_diagnostic(
                "KWin refused to load the current-run shortcut script",
            ));
        }
        proxy.call::<_, _, ()>("start", &()).map_err(bridge_error)?;

        self.actions.insert(action_name.clone(), hotkey.clone());
        self.scripts.push(KwinScriptHandle {
            action_name,
            script_id: script_id.clone(),
            script_path,
        });
        Ok(format!("kde-kwin-script:{script_id}:{}", hotkey.as_str()))
    }

    pub fn ensure_active_process_monitor(&mut self) -> Result<(), DiagnosableError> {
        if self.active_process_monitor.is_some() {
            return Ok(());
        }
        self.next_index += 1;
        let action_name = format!(
            "SignalAurasActiveProcess_{}_{}",
            std::process::id(),
            self.next_index
        );
        let script_id = format!(
            "signal-auras-active-process-{}-{}",
            std::process::id(),
            self.next_index
        );
        let script_path = std::env::temp_dir().join(format!("{script_id}.js"));
        let script = kwin_active_process_script(
            &action_name,
            &self.callback_bus_name,
            &self.callback_object_path,
        );
        fs::write(&script_path, script).map_err(bridge_error)?;

        let proxy = kwin_scripting_proxy(&self.connection)?;
        let loaded_id: i32 = proxy
            .call(
                "loadScript",
                &(script_path.to_string_lossy().as_ref(), script_id.as_str()),
            )
            .map_err(bridge_error)?;
        if loaded_id < 0 {
            let _ = fs::remove_file(&script_path);
            return Err(bridge_diagnostic(
                "KWin refused to load the active-process metadata script",
            ));
        }
        proxy.call::<_, _, ()>("start", &()).map_err(bridge_error)?;

        self.active_process_monitor = Some(KwinScriptHandle {
            action_name,
            script_id,
            script_path,
        });
        Ok(())
    }

    pub fn callback_wake_fd(&self) -> RawFd {
        self.callback_wake_fd.as_raw_fd()
    }

    pub fn drain_callback_wake_fd(&self) -> Result<bool, DiagnosableError> {
        self.callback_wake_fd.drain()
    }

    pub fn take_callback_dropped_count(&mut self) -> u64 {
        self.queue
            .lock()
            .map(|mut queue| queue.take_dropped_count())
            .unwrap_or_default()
    }

    pub fn next_shortcut_event(&mut self) -> Option<ObservedShortcutEvent> {
        while let Some(event) = self
            .queue
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front())
        {
            self.active_process = event.active_process;
            if let Some(hotkey) = self.actions.get(&event.action_name) {
                return Some(ObservedShortcutEvent {
                    hotkey: hotkey.clone(),
                    received_at: event.received_at,
                });
            }
        }
        None
    }

    pub fn active_process_context(&self) -> ActiveProcessContext {
        let mut context = self.active_process.clone();
        context.captured_at = Instant::now();
        context
    }

    pub fn unload(&mut self) -> Result<CleanupReport, DiagnosableError> {
        let attempted = self.scripts.len();
        let attempted = attempted + usize::from(self.active_process_monitor.is_some());
        if attempted == 0 {
            return Ok(CleanupReport::empty());
        }
        let proxy = kwin_scripting_proxy(&self.connection)?;
        let kglobalaccel = kglobalaccel_proxy(&self.connection)?;
        let mut failures = 0usize;
        let mut scripts = self.scripts.drain(..).collect::<Vec<_>>();
        if let Some(script) = self.active_process_monitor.take() {
            scripts.push(script);
        }
        for script in scripts {
            let unloaded = proxy
                .call::<_, _, bool>("unloadScript", &script.script_id.as_str())
                .unwrap_or(false);
            let unregistered = if self.actions.contains_key(&script.action_name) {
                kglobalaccel
                    .call::<_, _, bool>("unregister", &("kwin", script.action_name.as_str()))
                    .unwrap_or(false)
            } else {
                true
            };
            if !unloaded || !unregistered {
                failures += 1;
            }
            let _ = fs::remove_file(script.script_path);
        }
        self.actions.clear();
        Ok(CleanupReport {
            attempted,
            succeeded: attempted.saturating_sub(failures),
            failed: failures,
        })
    }
}

impl Drop for KwinShortcutBridge {
    fn drop(&mut self) {
        let _ = self.unload();
    }
}

#[derive(Debug, Clone)]
struct KwinBridgeEvent {
    action_name: String,
    active_process: ActiveProcessContext,
    received_at: Instant,
}

#[derive(Debug)]
struct KwinCallbackQueue {
    capacity: usize,
    events: VecDeque<KwinBridgeEvent>,
    dropped_count: u64,
}

impl KwinCallbackQueue {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::new(),
            dropped_count: 0,
        }
    }

    fn push(&mut self, event: KwinBridgeEvent) -> bool {
        if self.events.len() >= self.capacity {
            self.dropped_count += 1;
            return false;
        }
        self.events.push_back(event);
        true
    }

    fn pop_front(&mut self) -> Option<KwinBridgeEvent> {
        self.events.pop_front()
    }

    fn take_dropped_count(&mut self) -> u64 {
        let count = self.dropped_count;
        self.dropped_count = 0;
        count
    }
}

fn spawn_kwin_callback_listener(
    bus_name: &str,
    object_path: &str,
    queue: Arc<Mutex<KwinCallbackQueue>>,
    wake_sender: crate::event_loop::RuntimeWakeSender,
) -> Result<(), DiagnosableError> {
    let connection = zbus::blocking::Connection::session().map_err(bridge_error)?;
    connection.request_name(bus_name).map_err(bridge_error)?;
    let object_path = object_path.to_string();
    thread::spawn(move || {
        let mut messages = zbus::blocking::MessageIterator::from(&connection);
        for message in &mut messages {
            let Ok(message) = message else {
                continue;
            };
            let header = message.header();
            if header.message_type() != zbus::message::Type::MethodCall
                || header.member().map(|m| m.as_str()) != Some("triggered")
                || header.interface().map(|i| i.as_str()) != Some("org.signalAuras.KWinBridge")
                || header.path().map(|p| p.as_str()) != Some(object_path.as_str())
            {
                continue;
            }
            let Ok((action_name, visible_name, app_id, window_class, pid)) = message
                .body()
                .deserialize::<(String, String, String, String, String)>()
            else {
                let _ = connection.reply(&header, &());
                continue;
            };
            let event = KwinBridgeEvent {
                action_name,
                active_process: kwin_callback_context(
                    visible_name,
                    app_id,
                    window_class,
                    pid.parse::<u32>().unwrap_or_default(),
                ),
                received_at: Instant::now(),
            };
            if let Ok(mut queue) = queue.lock() {
                queue.push(event);
            }
            let _ = wake_sender.wake();
            let _ = connection.reply(&header, &());
        }
    });
    Ok(())
}

fn kwin_callback_context(
    visible_name: String,
    app_id: String,
    window_class: String,
    pid: u32,
) -> ActiveProcessContext {
    let matchable_name = first_non_empty([
        app_id.as_str(),
        window_class.as_str(),
        visible_name.as_str(),
    ]);
    let Some(matchable_name) = matchable_name else {
        return ActiveProcessContext::unavailable("KDE active window metadata is unavailable");
    };
    let Ok(process_name) = ProcessName::parse(matchable_name) else {
        return ActiveProcessContext::unavailable("KDE active window metadata is invalid");
    };
    let mut context = if pid > 0 || !app_id.is_empty() {
        ActiveProcessContext::exact(process_name, (pid > 0).then_some(pid))
    } else {
        ActiveProcessContext::name_only(process_name)
    };
    if !app_id.is_empty() {
        context = context.with_app_id(app_id);
    }
    if !window_class.is_empty() {
        context = context.with_window_class(window_class);
    }
    context
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    values
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn kwin_scripting_proxy(
    connection: &zbus::blocking::Connection,
) -> Result<zbus::blocking::Proxy<'_>, DiagnosableError> {
    zbus::blocking::Proxy::new(
        connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .map_err(bridge_error)
}

fn kglobalaccel_proxy(
    connection: &zbus::blocking::Connection,
) -> Result<zbus::blocking::Proxy<'_>, DiagnosableError> {
    zbus::blocking::Proxy::new(
        connection,
        "org.kde.kglobalaccel",
        "/kglobalaccel",
        "org.kde.KGlobalAccel",
    )
    .map_err(bridge_error)
}

fn kwin_shortcut_script(
    action_name: &str,
    shortcut: &str,
    description: &str,
    bus_name: &str,
    object_path: &str,
) -> String {
    format!(
        "function signalAurasValue(value) {{ return value === undefined || value === null ? \"\" : value.toString(); }}\n\
         registerShortcut({action:?}, {description:?}, {shortcut:?}, function() {{\n\
             var window = workspace.activeWindow;\n\
             var caption = \"\";\n\
             var appId = \"\";\n\
             var windowClass = \"\";\n\
             var pid = \"\";\n\
             try {{\n\
                 if (window) {{\n\
                     caption = signalAurasValue(window.caption);\n\
                     appId = signalAurasValue(window.resourceClass);\n\
                     windowClass = signalAurasValue(window.windowClass);\n\
                     pid = signalAurasValue(window.pid);\n\
                 }}\n\
             }} catch (error) {{\n\
             }}\n\
             callDBus({bus:?}, {path:?}, \"org.signalAuras.KWinBridge\", \"triggered\", {action:?}, caption, appId, windowClass, pid);\n\
         }});\n",
        action = action_name,
        description = description,
        shortcut = shortcut,
        bus = bus_name,
        path = object_path,
    )
}

fn kwin_active_process_script(action_name: &str, bus_name: &str, object_path: &str) -> String {
    format!(
        "function signalAurasValue(value) {{ return value === undefined || value === null ? \"\" : value.toString(); }}\n\
         function signalAurasReportActiveWindow() {{\n\
             var window = workspace.activeWindow;\n\
             var caption = \"\";\n\
             var appId = \"\";\n\
             var windowClass = \"\";\n\
             var pid = \"\";\n\
             try {{\n\
                 if (window) {{\n\
                     caption = signalAurasValue(window.caption);\n\
                     appId = signalAurasValue(window.resourceClass);\n\
                     windowClass = signalAurasValue(window.windowClass);\n\
                     pid = signalAurasValue(window.pid);\n\
                 }}\n\
             }} catch (error) {{\n\
             }}\n\
             callDBus({bus:?}, {path:?}, \"org.signalAuras.KWinBridge\", \"triggered\", {action:?}, caption, appId, windowClass, pid);\n\
         }}\n\
         signalAurasReportActiveWindow();\n\
         workspace.windowActivated.connect(function(window) {{ signalAurasReportActiveWindow(); }});\n",
        action = action_name,
        bus = bus_name,
        path = object_path,
    )
}

fn bridge_error(error: impl std::fmt::Display) -> DiagnosableError {
    bridge_diagnostic(format!("{error}"))
}

fn bridge_diagnostic(message: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::Registration, message)
        .with_source("kwin-scripting")
        .with_remediation(
            "verify KWin scripting and KGlobalAccel are available in this KDE session",
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_queue_preserves_accepted_arrival_order() {
        let mut queue = KwinCallbackQueue::new(3);
        for action in ["one", "two", "three"] {
            assert!(queue.push(event(action)));
        }

        assert_eq!(queue.pop_front().unwrap().action_name, "one");
        assert_eq!(queue.pop_front().unwrap().action_name, "two");
        assert_eq!(queue.pop_front().unwrap().action_name, "three");
        assert!(queue.pop_front().is_none());
        assert_eq!(queue.take_dropped_count(), 0);
    }

    #[test]
    fn callback_queue_drops_newest_when_full_and_reports_count() {
        let mut queue = KwinCallbackQueue::new(2);

        assert!(queue.push(event("one")));
        assert!(queue.push(event("two")));
        assert!(!queue.push(event("three")));
        assert!(!queue.push(event("four")));

        assert_eq!(queue.pop_front().unwrap().action_name, "one");
        assert_eq!(queue.pop_front().unwrap().action_name, "two");
        assert!(queue.pop_front().is_none());
        assert_eq!(queue.take_dropped_count(), 2);
        assert_eq!(queue.take_dropped_count(), 0);
    }

    fn event(action_name: &str) -> KwinBridgeEvent {
        KwinBridgeEvent {
            action_name: action_name.to_string(),
            active_process: ActiveProcessContext::unavailable("test"),
            received_at: Instant::now(),
        }
    }
}
