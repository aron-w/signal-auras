use crate::prompt::{DevicePromptCandidate, DeviceSelectionDecision, ScopePrompt};
use signal_auras_core::{DiagnosableError, ErrorPhase, InputProviderConfig, InputProviderOutput};
use signal_auras_wayland::evdev::{
    evdev_device_identity, evdev_device_name, EvdevDeviceIdentity, SIGNAL_AURAS_UINPUT_DEVICE_NAME,
};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAccessStatus {
    Accessible,
    Missing(String),
    Denied(String),
}

impl InputAccessStatus {
    pub fn is_accessible(&self) -> bool {
        matches!(self, Self::Accessible)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Accessible => "ok",
            Self::Missing(_) => "missing",
            Self::Denied(_) => "denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheValidationStatus {
    Accepted,
    Missing,
    Invalid(String),
    Stale(String),
    PermissionIncomplete(String),
    UnsafeRuntimeDir(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheValidationReport {
    pub cache_path: PathBuf,
    pub status: CacheValidationStatus,
    pub selected_devices: Vec<PathBuf>,
}

impl CacheValidationReport {
    pub fn accepted(cache_path: PathBuf, selected_devices: Vec<PathBuf>) -> Self {
        Self {
            cache_path,
            status: CacheValidationStatus::Accepted,
            selected_devices,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveDeviceCandidate {
    pub path: PathBuf,
    pub label: String,
    pub identity: Option<EvdevDeviceIdentity>,
    pub access: InputAccessStatus,
    pub stable_path: Option<PathBuf>,
    pub self_generated: bool,
}

pub trait InputDeviceProbe {
    fn event_devices(&self) -> Result<Vec<PathBuf>, DiagnosableError>;
    fn read_access(&self, path: &Path) -> InputAccessStatus;
    fn read_write_access(&self, path: &Path) -> InputAccessStatus;
    fn symlink_target(&self, path: &Path) -> Option<PathBuf>;
    fn stable_path_for(&self, path: &Path) -> Option<PathBuf>;
    fn device_name(&self, path: &Path) -> Option<String>;
    fn device_identity(&self, path: &Path) -> Option<EvdevDeviceIdentity>;
}

pub trait PermissionRepair {
    fn repair(&mut self, evdev_paths: &[PathBuf], uinput: bool) -> Result<(), DiagnosableError>;
}

pub struct RealInputDeviceProbe;

impl InputDeviceProbe for RealInputDeviceProbe {
    fn event_devices(&self) -> Result<Vec<PathBuf>, DiagnosableError> {
        signal_auras_wayland::evdev::discover_event_devices()
    }

    fn read_access(&self, path: &Path) -> InputAccessStatus {
        match OpenOptions::new().read(true).open(path) {
            Ok(_) => InputAccessStatus::Accessible,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                InputAccessStatus::Missing(error.to_string())
            }
            Err(error) => InputAccessStatus::Denied(error.to_string()),
        }
    }

    fn read_write_access(&self, path: &Path) -> InputAccessStatus {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(_) => InputAccessStatus::Accessible,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                InputAccessStatus::Missing(error.to_string())
            }
            Err(error) => InputAccessStatus::Denied(error.to_string()),
        }
    }

    fn symlink_target(&self, path: &Path) -> Option<PathBuf> {
        fs::read_link(path).ok()
    }

    fn stable_path_for(&self, path: &Path) -> Option<PathBuf> {
        stable_signal_auras_path_for(path)
    }

    fn device_name(&self, path: &Path) -> Option<String> {
        evdev_device_name(path)
    }

    fn device_identity(&self, path: &Path) -> Option<EvdevDeviceIdentity> {
        evdev_device_identity(path)
    }
}

pub struct SudoSetfaclRepair;

impl PermissionRepair for SudoSetfaclRepair {
    fn repair(&mut self, evdev_paths: &[PathBuf], uinput: bool) -> Result<(), DiagnosableError> {
        if uinput {
            let _ = Command::new("sudo").args(["modprobe", "uinput"]).status();
        }
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        for path in evdev_paths {
            run_acl_command("r", &user, path)?;
        }
        if uinput {
            run_acl_command("rw", &user, Path::new("/dev/uinput"))?;
        }
        Ok(())
    }
}

fn run_acl_command(mode: &str, user: &str, path: &Path) -> Result<(), DiagnosableError> {
    let status = Command::new("sudo")
        .arg("setfacl")
        .arg("-m")
        .arg(format!("u:{user}:{mode}"))
        .arg(path)
        .status()
        .map_err(|error| {
            DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!("cannot run sudo setfacl for '{}': {error}", path.display()),
            )
        })?;
    if !status.success() {
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            format!("sudo setfacl failed for '{}'", path.display()),
        ));
    }
    Ok(())
}

pub fn resolve_interactive_input_provider(
    lua_file: &Path,
    provider: &InputProviderConfig,
    prompt: &mut impl ScopePrompt,
    probe: &impl InputDeviceProbe,
    repair: &mut impl PermissionRepair,
) -> Result<InputProviderConfig, DiagnosableError> {
    if !provider.interactive_devices {
        return Ok(provider.clone());
    }

    let cache_path = runtime_cache_path(lua_file)?;
    match validate_cache_file(&cache_path, lua_file, provider, probe) {
        Ok(report) if matches!(report.status, CacheValidationStatus::Accepted) => {
            return provider.with_selected_devices(report.selected_devices);
        }
        Ok(_) | Err(_) => {}
    }

    let candidates = interactive_candidates(probe)?;
    let prompt_candidates = candidates
        .iter()
        .map(|candidate| DevicePromptCandidate {
            path: candidate
                .stable_path
                .clone()
                .unwrap_or_else(|| candidate.path.clone()),
            label: candidate.label.clone(),
            selected: false,
        })
        .collect::<Vec<_>>();
    let decision =
        prompt.select_input_devices("missing or stale runtime cache", &prompt_candidates)?;
    let selected = match decision {
        DeviceSelectionDecision::Selected(paths) => paths,
        DeviceSelectionDecision::Cancel => {
            return Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "interactive input device selection cancelled",
            ));
        }
        DeviceSelectionDecision::NonInteractive => {
            return Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "interactive input device selection requires interactive stdin or a valid runtime cache",
            ));
        }
    };

    let mut permission_incomplete = selected
        .iter()
        .any(|path| !probe.read_access(path).is_accessible());
    let needs_uinput = provider.output == InputProviderOutput::Uinput;
    if needs_uinput
        && !probe
            .read_write_access(Path::new("/dev/uinput"))
            .is_accessible()
    {
        permission_incomplete = true;
    }
    if permission_incomplete && prompt.confirm_input_permission_repair(&selected, needs_uinput)? {
        repair.repair(&selected, needs_uinput)?;
    }

    validate_selected_devices(&selected, provider, probe)?;
    write_cache_file(&cache_path, lua_file, provider, &selected, probe)?;
    provider.with_selected_devices(selected)
}

pub fn validate_cache_file(
    cache_path: &Path,
    lua_file: &Path,
    provider: &InputProviderConfig,
    probe: &impl InputDeviceProbe,
) -> Result<CacheValidationReport, DiagnosableError> {
    if !cache_path.exists() {
        return Ok(CacheValidationReport {
            cache_path: cache_path.to_path_buf(),
            status: CacheValidationStatus::Missing,
            selected_devices: Vec::new(),
        });
    }
    let cache = RuntimeDeviceCache::read(cache_path)?;
    let canonical = canonical_main_lua_path(lua_file)?;
    if cache.version != CACHE_VERSION {
        return Ok(invalid(cache_path, "unsupported cache version"));
    }
    if cache.script_path != canonical {
        return Ok(invalid(
            cache_path,
            "cache belongs to a different script path",
        ));
    }
    if cache.mode != format!("{:?}", provider.mode)
        || cache.output != format!("{:?}", provider.output)
    {
        return Ok(invalid(cache_path, "cache provider mode or output changed"));
    }
    let selected = cache
        .devices
        .iter()
        .map(|device| device.path.clone())
        .collect::<Vec<_>>();
    if let Err(error) = validate_selected_devices(&selected, provider, probe) {
        return Ok(CacheValidationReport {
            cache_path: cache_path.to_path_buf(),
            status: CacheValidationStatus::PermissionIncomplete(error.message),
            selected_devices: selected,
        });
    }
    for cached in &cache.devices {
        let Some(current) = probe.device_identity(&cached.path) else {
            return Ok(CacheValidationReport {
                cache_path: cache_path.to_path_buf(),
                status: CacheValidationStatus::Stale(format!(
                    "missing current identity for '{}'",
                    cached.path.display()
                )),
                selected_devices: selected,
            });
        };
        if current != cached.identity {
            return Ok(CacheValidationReport {
                cache_path: cache_path.to_path_buf(),
                status: CacheValidationStatus::Stale(format!(
                    "device identity changed for '{}'",
                    cached.path.display()
                )),
                selected_devices: selected,
            });
        }
    }
    Ok(CacheValidationReport::accepted(
        cache_path.to_path_buf(),
        selected,
    ))
}

pub fn runtime_cache_path(lua_file: &Path) -> Result<PathBuf, DiagnosableError> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "XDG_RUNTIME_DIR is required for interactive input device cache",
        )
    })?;
    let runtime = PathBuf::from(runtime);
    if !runtime.is_absolute() {
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "XDG_RUNTIME_DIR must be an absolute path for interactive input device cache",
        ));
    }
    validate_runtime_dir(&runtime)?;
    let canonical = canonical_main_lua_path(lua_file)?;
    let key = cache_key(canonical.to_string_lossy().as_bytes());
    Ok(runtime
        .join("signal-auras")
        .join("input-devices")
        .join(format!("{key}.cache")))
}

fn validate_runtime_dir(runtime: &Path) -> Result<(), DiagnosableError> {
    let metadata = fs::metadata(runtime).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            format!(
                "cannot inspect XDG_RUNTIME_DIR '{}' for interactive input device cache: {error}",
                runtime.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "XDG_RUNTIME_DIR must be a directory for interactive input device cache",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Some(uid) = current_uid() {
            if metadata.uid() != uid {
                return Err(DiagnosableError::new(
                    ErrorPhase::CapabilityProbe,
                    "XDG_RUNTIME_DIR must be owned by the current user for interactive input device cache",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn current_uid() -> Option<u32> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|line| line.split_whitespace().next())
        .and_then(|uid| uid.parse::<u32>().ok())
}

pub fn interactive_cache_report(
    lua_file: &Path,
    provider: &InputProviderConfig,
    probe: &impl InputDeviceProbe,
) -> CacheValidationReport {
    match runtime_cache_path(lua_file) {
        Ok(path) => validate_cache_file(&path, lua_file, provider, probe).unwrap_or_else(|error| {
            CacheValidationReport {
                cache_path: path,
                status: CacheValidationStatus::Invalid(error.message),
                selected_devices: Vec::new(),
            }
        }),
        Err(error) => CacheValidationReport {
            cache_path: PathBuf::new(),
            status: CacheValidationStatus::UnsafeRuntimeDir(error.message),
            selected_devices: Vec::new(),
        },
    }
}

pub fn interactive_candidates(
    probe: &impl InputDeviceProbe,
) -> Result<Vec<InteractiveDeviceCandidate>, DiagnosableError> {
    let mut candidates = probe
        .event_devices()?
        .into_iter()
        .map(|path| {
            let identity = probe.device_identity(&path);
            let name = probe
                .device_name(&path)
                .or_else(|| identity.as_ref().and_then(|identity| identity.name.clone()));
            let self_generated = name
                .as_deref()
                .is_some_and(|name| name == SIGNAL_AURAS_UINPUT_DEVICE_NAME);
            let access = probe.read_access(&path);
            let stable_path = probe.stable_path_for(&path);
            let label = format!(
                "name={} access={} stable={}",
                name.unwrap_or_else(|| "unknown".to_string())
                    .replace(' ', "_"),
                access.label(),
                stable_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
            InteractiveDeviceCandidate {
                path,
                label,
                identity,
                access,
                stable_path,
                self_generated,
            }
        })
        .filter(|candidate| !candidate.self_generated)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(candidates)
}

fn validate_selected_devices(
    selected: &[PathBuf],
    provider: &InputProviderConfig,
    probe: &impl InputDeviceProbe,
) -> Result<(), DiagnosableError> {
    if selected.is_empty() {
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "interactive input device selection requires at least one selected device",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for path in selected {
        if !seen.insert(path.clone()) {
            return Err(DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!("duplicate selected evdev input device '{}'", path.display()),
            ));
        }
        if !probe.read_access(path).is_accessible() {
            return Err(DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!(
                    "selected evdev input device '{}' is not readable",
                    path.display()
                ),
            ));
        }
        if probe
            .device_name(path)
            .as_deref()
            .is_some_and(|name| name == SIGNAL_AURAS_UINPUT_DEVICE_NAME)
        {
            return Err(DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!(
                    "selected evdev input device '{}' is self-generated",
                    path.display()
                ),
            ));
        }
        if probe.device_identity(path).is_none() {
            return Err(DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!(
                    "selected evdev input device '{}' has no stable identity",
                    path.display()
                ),
            ));
        }
    }
    if provider.output == InputProviderOutput::Uinput
        && !probe
            .read_write_access(Path::new("/dev/uinput"))
            .is_accessible()
    {
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "uinput output requires read/write access to /dev/uinput",
        ));
    }
    Ok(())
}

fn write_cache_file(
    cache_path: &Path,
    lua_file: &Path,
    provider: &InputProviderConfig,
    selected: &[PathBuf],
    probe: &impl InputDeviceProbe,
) -> Result<(), DiagnosableError> {
    let parent = cache_path.parent().ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "cache path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            format!("cannot create interactive input cache directory: {error}"),
        )
    })?;
    let cache = RuntimeDeviceCache {
        version: CACHE_VERSION.to_string(),
        script_path: canonical_main_lua_path(lua_file)?,
        mode: format!("{:?}", provider.mode),
        output: format!("{:?}", provider.output),
        updated_unix_ms: current_unix_ms(),
        devices: selected
            .iter()
            .map(|path| {
                let identity = probe.device_identity(path).ok_or_else(|| {
                    DiagnosableError::new(
                        ErrorPhase::CapabilityProbe,
                        format!(
                            "selected evdev input device '{}' has no stable identity",
                            path.display()
                        ),
                    )
                })?;
                Ok(CachedDevice {
                    path: path.clone(),
                    identity,
                })
            })
            .collect::<Result<Vec<_>, DiagnosableError>>()?,
    };
    fs::write(cache_path, cache.render()).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            format!(
                "cannot write interactive input cache '{}': {error}",
                cache_path.display()
            ),
        )
    })
}

fn invalid(cache_path: &Path, message: &str) -> CacheValidationReport {
    CacheValidationReport {
        cache_path: cache_path.to_path_buf(),
        status: CacheValidationStatus::Invalid(message.to_string()),
        selected_devices: Vec::new(),
    }
}

fn canonical_main_lua_path(path: &Path) -> Result<PathBuf, DiagnosableError> {
    fs::canonicalize(path).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!("cannot canonicalize Lua file '{}': {error}", path.display()),
        )
    })
}

fn cache_key(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDeviceCache {
    version: String,
    script_path: PathBuf,
    mode: String,
    output: String,
    updated_unix_ms: u128,
    devices: Vec<CachedDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedDevice {
    path: PathBuf,
    identity: EvdevDeviceIdentity,
}

impl RuntimeDeviceCache {
    fn read(path: &Path) -> Result<Self, DiagnosableError> {
        let source = fs::read_to_string(path).map_err(|error| {
            DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                format!(
                    "cannot read interactive input cache '{}': {error}",
                    path.display()
                ),
            )
        })?;
        Self::parse(&source)
    }

    fn parse(source: &str) -> Result<Self, DiagnosableError> {
        let mut version = None;
        let mut script_path = None;
        let mut mode = None;
        let mut output = None;
        let mut updated_unix_ms = None;
        let mut devices = Vec::new();
        for line in source.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "version" => version = Some(value.to_string()),
                "script" => script_path = Some(PathBuf::from(hex_decode_string(value)?)),
                "mode" => mode = Some(hex_decode_string(value)?),
                "output" => output = Some(hex_decode_string(value)?),
                "updated_unix_ms" => updated_unix_ms = value.parse::<u128>().ok(),
                "device" => devices.push(parse_cached_device(value)?),
                _ => {}
            }
        }
        Ok(Self {
            version: version.ok_or_else(cache_parse_error)?,
            script_path: script_path.ok_or_else(cache_parse_error)?,
            mode: mode.ok_or_else(cache_parse_error)?,
            output: output.ok_or_else(cache_parse_error)?,
            updated_unix_ms: updated_unix_ms.unwrap_or_default(),
            devices,
        })
    }

    fn render(&self) -> String {
        let mut lines = vec![
            format!("version={}", self.version),
            format!(
                "script={}",
                hex_encode(self.script_path.to_string_lossy().as_bytes())
            ),
            format!("mode={}", hex_encode(self.mode.as_bytes())),
            format!("output={}", hex_encode(self.output.as_bytes())),
            format!("updated_unix_ms={}", self.updated_unix_ms),
        ];
        lines.extend(self.devices.iter().map(render_cached_device));
        lines.push(String::new());
        lines.join("\n")
    }
}

fn render_cached_device(device: &CachedDevice) -> String {
    let fields = [
        device.path.to_string_lossy().to_string(),
        device.identity.event_name.clone(),
        device.identity.name.clone().unwrap_or_default(),
        device.identity.phys.clone().unwrap_or_default(),
        device.identity.uniq.clone().unwrap_or_default(),
        device.identity.bustype.clone().unwrap_or_default(),
        device.identity.vendor.clone().unwrap_or_default(),
        device.identity.product.clone().unwrap_or_default(),
        device.identity.version.clone().unwrap_or_default(),
    ];
    format!(
        "device={}",
        fields
            .iter()
            .map(|field| hex_encode(field.as_bytes()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn parse_cached_device(value: &str) -> Result<CachedDevice, DiagnosableError> {
    let fields = value
        .split(',')
        .map(hex_decode_string)
        .collect::<Result<Vec<_>, _>>()?;
    if fields.len() != 9 {
        return Err(cache_parse_error());
    }
    Ok(CachedDevice {
        path: PathBuf::from(&fields[0]),
        identity: EvdevDeviceIdentity {
            event_name: fields[1].clone(),
            name: nonempty(fields[2].clone()),
            phys: nonempty(fields[3].clone()),
            uniq: nonempty(fields[4].clone()),
            bustype: nonempty(fields[5].clone()),
            vendor: nonempty(fields[6].clone()),
            product: nonempty(fields[7].clone()),
            version: nonempty(fields[8].clone()),
        },
    })
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn cache_parse_error() -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "invalid interactive input cache",
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode_string(value: &str) -> Result<String, DiagnosableError> {
    if !value.len().is_multiple_of(2) {
        return Err(cache_parse_error());
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| cache_parse_error())
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|_| cache_parse_error())
}

fn stable_signal_auras_path_for(path: &Path) -> Option<PathBuf> {
    let directory = Path::new("/dev/input/by-signal-auras");
    let entries = fs::read_dir(directory).ok()?;
    let canonical_path = fs::canonicalize(path).ok();
    for entry in entries.filter_map(Result::ok) {
        let stable_path = entry.path();
        let Some(target) = fs::read_link(&stable_path).ok() else {
            continue;
        };
        let resolved = if target.is_absolute() {
            target
        } else {
            directory.join(target)
        };
        if resolved == path
            || canonical_path
                .as_ref()
                .is_some_and(|path| fs::canonicalize(&resolved).ok().as_ref() == Some(path))
        {
            return Some(stable_path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{InputProviderMode, InputProviderOutput};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeProbe {
        devices: Vec<PathBuf>,
        read: BTreeMap<PathBuf, InputAccessStatus>,
        read_write: BTreeMap<PathBuf, InputAccessStatus>,
        identities: BTreeMap<PathBuf, EvdevDeviceIdentity>,
        names: BTreeMap<PathBuf, String>,
        stable: BTreeMap<PathBuf, PathBuf>,
    }

    impl InputDeviceProbe for FakeProbe {
        fn event_devices(&self) -> Result<Vec<PathBuf>, DiagnosableError> {
            Ok(self.devices.clone())
        }

        fn read_access(&self, path: &Path) -> InputAccessStatus {
            self.read.get(path).cloned().unwrap_or_else(|| {
                InputAccessStatus::Missing("No such file or directory".to_string())
            })
        }

        fn read_write_access(&self, path: &Path) -> InputAccessStatus {
            self.read_write.get(path).cloned().unwrap_or_else(|| {
                InputAccessStatus::Missing("No such file or directory".to_string())
            })
        }

        fn symlink_target(&self, _path: &Path) -> Option<PathBuf> {
            None
        }

        fn stable_path_for(&self, path: &Path) -> Option<PathBuf> {
            self.stable.get(path).cloned()
        }

        fn device_name(&self, path: &Path) -> Option<String> {
            self.names.get(path).cloned()
        }

        fn device_identity(&self, path: &Path) -> Option<EvdevDeviceIdentity> {
            self.identities.get(path).cloned()
        }
    }

    #[derive(Default)]
    struct FakeRepair {
        calls: Vec<(Vec<PathBuf>, bool)>,
    }

    impl PermissionRepair for FakeRepair {
        fn repair(
            &mut self,
            evdev_paths: &[PathBuf],
            uinput: bool,
        ) -> Result<(), DiagnosableError> {
            self.calls.push((evdev_paths.to_vec(), uinput));
            Ok(())
        }
    }

    struct FakePrompt {
        selection: DeviceSelectionDecision,
        repair: bool,
        select_calls: usize,
    }

    impl ScopePrompt for FakePrompt {
        fn resolve_missing_scope(
            &mut self,
        ) -> Result<signal_auras_core::ConsentDecision, DiagnosableError> {
            Ok(signal_auras_core::ConsentDecision::Cancel)
        }

        fn select_input_devices(
            &mut self,
            _reason: &str,
            _candidates: &[DevicePromptCandidate],
        ) -> Result<DeviceSelectionDecision, DiagnosableError> {
            self.select_calls += 1;
            Ok(self.selection.clone())
        }

        fn confirm_input_permission_repair(
            &mut self,
            _paths: &[PathBuf],
            _uinput: bool,
        ) -> Result<bool, DiagnosableError> {
            Ok(self.repair)
        }
    }

    #[test]
    fn cache_key_uses_canonical_lua_path_under_runtime_dir() {
        let _guard = env_lock().lock().unwrap();
        let runtime = temp_dir("runtime-key");
        let script_dir = temp_dir("script-key");
        let script = script_dir.join("main.lua");
        fs::write(&script, "return {}").unwrap();
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);

        let path = runtime_cache_path(&script).unwrap();

        assert!(path.starts_with(runtime.join("signal-auras/input-devices")));
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("cache"));
        restore_runtime_dir(previous);
    }

    #[test]
    fn valid_cache_resolves_without_prompt() {
        let _guard = env_lock().lock().unwrap();
        let runtime = temp_dir("valid-cache");
        let script = write_script("valid-cache");
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        let provider = interactive_provider();
        let device = PathBuf::from("/dev/input/event3");
        let mut probe = FakeProbe::default();
        probe.devices.push(device.clone());
        probe
            .read
            .insert(device.clone(), InputAccessStatus::Accessible);
        probe
            .read_write
            .insert(PathBuf::from("/dev/uinput"), InputAccessStatus::Accessible);
        probe
            .identities
            .insert(device.clone(), identity("event3", "keyboard"));
        write_cache_file(
            &runtime_cache_path(&script).unwrap(),
            &script,
            &provider,
            &[device.clone()],
            &probe,
        )
        .unwrap();
        let mut prompt = FakePrompt {
            selection: DeviceSelectionDecision::NonInteractive,
            repair: false,
            select_calls: 0,
        };
        let mut repair = FakeRepair::default();

        let resolved = resolve_interactive_input_provider(
            &script,
            &provider,
            &mut prompt,
            &probe,
            &mut repair,
        )
        .unwrap();

        assert_eq!(resolved.devices, vec![device]);
        assert!(!resolved.interactive_devices);
        assert_eq!(prompt.select_calls, 0);
        restore_runtime_dir(previous);
    }

    #[test]
    fn stale_cache_reprompts_and_rewrites_selection() {
        let _guard = env_lock().lock().unwrap();
        let runtime = temp_dir("stale-cache");
        let script = write_script("stale-cache");
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        let provider = interactive_provider();
        let device = PathBuf::from("/dev/input/event3");
        let mut cache_probe = FakeProbe::default();
        cache_probe
            .read
            .insert(device.clone(), InputAccessStatus::Accessible);
        cache_probe
            .read_write
            .insert(PathBuf::from("/dev/uinput"), InputAccessStatus::Accessible);
        cache_probe
            .identities
            .insert(device.clone(), identity("event3", "old"));
        write_cache_file(
            &runtime_cache_path(&script).unwrap(),
            &script,
            &provider,
            std::slice::from_ref(&device),
            &cache_probe,
        )
        .unwrap();
        let mut probe = FakeProbe::default();
        probe.devices.push(device.clone());
        probe
            .read
            .insert(device.clone(), InputAccessStatus::Accessible);
        probe
            .read_write
            .insert(PathBuf::from("/dev/uinput"), InputAccessStatus::Accessible);
        probe
            .identities
            .insert(device.clone(), identity("event3", "new"));
        probe.names.insert(device.clone(), "new".to_string());
        let mut prompt = FakePrompt {
            selection: DeviceSelectionDecision::Selected(vec![device.clone()]),
            repair: false,
            select_calls: 0,
        };
        let mut repair = FakeRepair::default();

        let resolved = resolve_interactive_input_provider(
            &script,
            &provider,
            &mut prompt,
            &probe,
            &mut repair,
        )
        .unwrap();

        assert_eq!(resolved.devices, vec![device.clone()]);
        assert_eq!(prompt.select_calls, 1);
        assert!(matches!(
            validate_cache_file(
                &runtime_cache_path(&script).unwrap(),
                &script,
                &provider,
                &probe
            )
            .unwrap()
            .status,
            CacheValidationStatus::Accepted
        ));
        restore_runtime_dir(previous);
    }

    #[test]
    fn missing_cache_noninteractive_fails_closed() {
        let _guard = env_lock().lock().unwrap();
        let runtime = temp_dir("noninteractive");
        let script = write_script("noninteractive");
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        let provider = interactive_provider();
        let probe = FakeProbe::default();
        let mut prompt = FakePrompt {
            selection: DeviceSelectionDecision::NonInteractive,
            repair: false,
            select_calls: 0,
        };
        let mut repair = FakeRepair::default();

        let error = resolve_interactive_input_provider(
            &script,
            &provider,
            &mut prompt,
            &probe,
            &mut repair,
        )
        .unwrap_err();

        assert_eq!(error.phase, ErrorPhase::ScopePrompt);
        assert!(error.message.contains("interactive stdin"));
        restore_runtime_dir(previous);
    }

    #[test]
    fn permission_repair_is_scoped_to_selected_devices_and_uinput() {
        let _guard = env_lock().lock().unwrap();
        let runtime = temp_dir("repair");
        let script = write_script("repair");
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
        let provider = interactive_provider();
        let device = PathBuf::from("/dev/input/event9");
        let mut probe = FakeProbe::default();
        probe.devices.push(device.clone());
        probe.read.insert(
            device.clone(),
            InputAccessStatus::Denied("denied".to_string()),
        );
        probe.read_write.insert(
            PathBuf::from("/dev/uinput"),
            InputAccessStatus::Denied("denied".to_string()),
        );
        probe
            .identities
            .insert(device.clone(), identity("event9", "mouse"));
        probe.names.insert(device.clone(), "mouse".to_string());
        let mut prompt = FakePrompt {
            selection: DeviceSelectionDecision::Selected(vec![device.clone()]),
            repair: true,
            select_calls: 0,
        };
        let mut repair = FakeRepair::default();

        let error = resolve_interactive_input_provider(
            &script,
            &provider,
            &mut prompt,
            &probe,
            &mut repair,
        )
        .unwrap_err();

        assert_eq!(repair.calls, vec![(vec![device.clone()], true)]);
        assert!(error.message.contains("not readable"));
        restore_runtime_dir(previous);
    }

    fn interactive_provider() -> InputProviderConfig {
        InputProviderConfig::evdev_interactive(InputProviderMode::Grab, InputProviderOutput::Uinput)
            .unwrap()
    }

    fn identity(event: &str, name: &str) -> EvdevDeviceIdentity {
        EvdevDeviceIdentity {
            event_name: event.to_string(),
            name: Some(name.to_string()),
            phys: Some("usb/input0".to_string()),
            uniq: None,
            bustype: Some("0003".to_string()),
            vendor: Some("0001".to_string()),
            product: Some("0002".to_string()),
            version: Some("0001".to_string()),
        }
    }

    fn write_script(label: &str) -> PathBuf {
        let dir = temp_dir(label);
        let script = dir.join("main.lua");
        fs::write(&script, "return {}").unwrap();
        script
    }

    fn temp_dir(label: &str) -> PathBuf {
        static NEXT_DIR_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        let sequence = NEXT_DIR_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        path.push(format!(
            "signal-auras-input-cache-{label}-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn restore_runtime_dir(previous: Option<std::ffi::OsString>) {
        if let Some(previous) = previous {
            std::env::set_var("XDG_RUNTIME_DIR", previous);
        } else {
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
    }

    fn env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }
}
