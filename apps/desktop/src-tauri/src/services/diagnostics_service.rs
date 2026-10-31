//! Local-only diagnostic event recording and export.
//!
//! This is the support bundle behind Settings → Export diagnostics
//! It is deliberately **not** the SQLCipher database:
//! the moments a diagnostic record matters most — the database failing to open, an engine
//! refusing to launch — are exactly the ones where the encrypted store is unavailable.
//!
//! Privacy is structural, not a later regex pass:
//! - `component` / `action` / `severity` / `outcome` are enums, so an event **cannot** carry a
//!   free-text value in those fields — serde rejects any unknown variant on read.
//! - `code` is only ever written from [`veil_core::ErrorCode`] (a fixed dotted vocabulary) and
//!   is re-validated to that same shape before export.
//! - Nothing here performs network I/O, and the exporter never retains a second copy.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode};

/// Version of the diagnostic event / bundle schema. Bump when a field changes meaning so a later
/// reader cannot silently interpret an older export.
pub const DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;

/// Append-only event log name inside `<data_dir>/diagnostics/`.
const EVENTS_FILE: &str = "events.jsonl";
/// On-disk size cap. Rotation keeps the newest events that fit under it.
const MAX_LOG_BYTES: u64 = 1024 * 1024; // 1 MiB
/// Events older than this are excluded from preview and export.
const MAX_EVENT_AGE_DAYS: i64 = 7;

// ---- Allowlisted vocabulary (the whole surface) -----------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticComponent {
    Startup,
    Engine,
    Launch,
    Proxy,
    Extensions,
    Vault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticAction {
    AppStarted,
    Install,
    Uninstall,
    Launch,
    Stop,
    Test,
    Import,
    Unlock,
    Recover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticOutcome {
    Ok,
    Failed,
    Cancelled,
}

/// One recorded event. No free text anywhere: the four enum fields are the allowlist, and `code`
/// is a stable dotted error string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub schema_version: u32,
    /// Monotonic per log file, for ordering and de-duplication.
    pub seq: u64,
    /// RFC 3339 UTC timestamp.
    pub at: String,
    /// Milliseconds since this process started (a bounded, relative window).
    pub rel_ms: u64,
    pub component: DiagnosticComponent,
    pub action: DiagnosticAction,
    pub severity: DiagnosticSeverity,
    pub outcome: DiagnosticOutcome,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsManifest {
    pub schema_version: u32,
    pub app_version: String,
    pub engine_version: Option<String>,
    pub engine_flag_dialect: Option<String>,
    /// Real OS name + build (e.g. "Microsoft Windows 11 Pro (10.0.26100)"), falling back to
    /// "windows/x86_64" when the local query fails.
    pub windows_version: String,
    pub cpu_model: Option<String>,
    pub cpu_cores: Option<u32>,
    pub memory_bytes: Option<u64>,
    /// Populated only when the user opted in to graphics details.
    pub gpus: Vec<String>,
    pub exported_at: String,
    pub event_count: usize,
    pub includes_graphics_details: bool,
}

/// What `preview_diagnostics` returns and `export_diagnostics` writes — the same content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsBundle {
    pub manifest: DiagnosticsManifest,
    pub events: Vec<DiagnosticEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDiagnosticsResult {
    pub path: String,
    pub event_count: usize,
    pub bytes: u64,
}

/// Facts about the installed engine, resolved by the command layer from the runtime registry and
/// handed in so this service stays decoupled from runtime state.
#[derive(Debug, Clone, Default)]
pub struct EngineInfo {
    pub version: Option<String>,
    pub flag_dialect: Option<String>,
}

#[derive(Clone)]
pub struct DiagnosticsService {
    inner: Arc<Mutex<DiagnosticsState>>,
}

struct DiagnosticsState {
    dir: PathBuf,
    started_at: Instant,
    next_seq: u64,
}

impl DiagnosticsService {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("diagnostics");
        let _ = std::fs::create_dir_all(&dir);
        let next_seq = read_max_seq(&dir) + 1;
        Self {
            inner: Arc::new(Mutex::new(DiagnosticsState {
                dir,
                started_at: Instant::now(),
                next_seq,
            })),
        }
    }

    /// Append an event. Never fails: recording is best-effort and must not disturb the
    /// operation it is observing. `code` is a stable dotted error string (see
    /// [`code_is_allowlisted`], which re-validates the shape before export).
    pub fn record(
        &self,
        component: DiagnosticComponent,
        action: DiagnosticAction,
        severity: DiagnosticSeverity,
        outcome: DiagnosticOutcome,
        code: Option<String>,
    ) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let event = DiagnosticEvent {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            seq: state.next_seq,
            at: now_rfc3339(),
            rel_ms: state.started_at.elapsed().as_millis() as u64,
            component,
            action,
            severity,
            outcome,
            code,
        };
        state.next_seq += 1;
        let Ok(line) = serde_json::to_string(&event) else {
            return;
        };
        let path = state.dir.join(EVENTS_FILE);
        if append_line(&path, &line).is_err() {
            return;
        }
        if std::fs::metadata(&path)
            .map(|meta| meta.len() > MAX_LOG_BYTES)
            .unwrap_or(false)
        {
            let _ = trim_to_fit(&path, MAX_LOG_BYTES);
        }
    }

    /// Convenience: an operation finished cleanly.
    pub fn record_ok(&self, component: DiagnosticComponent, action: DiagnosticAction) {
        self.record(
            component,
            action,
            DiagnosticSeverity::Info,
            DiagnosticOutcome::Ok,
            None,
        );
    }

    /// Convenience: an operation failed with a stable error code.
    pub fn record_failure(
        &self,
        component: DiagnosticComponent,
        action: DiagnosticAction,
        code: ErrorCode,
    ) {
        self.record(
            component,
            action,
            DiagnosticSeverity::Error,
            DiagnosticOutcome::Failed,
            Some(code.as_str().to_string()),
        );
    }

    /// Convenience for callers that already hold a dotted code string (e.g.
    /// `LaunchStatus::Failed.code`). The shape is re-validated before export.
    pub fn record_failure_code(
        &self,
        component: DiagnosticComponent,
        action: DiagnosticAction,
        code: &str,
    ) {
        self.record(
            component,
            action,
            DiagnosticSeverity::Error,
            DiagnosticOutcome::Failed,
            Some(code.to_string()),
        );
    }

    /// Convenience: an operation was cancelled by the user.
    pub fn record_cancelled(&self, component: DiagnosticComponent, action: DiagnosticAction) {
        self.record(
            component,
            action,
            DiagnosticSeverity::Info,
            DiagnosticOutcome::Cancelled,
            None,
        );
    }

    /// Build the bundle the UI previews. This is exactly what `export` writes.
    pub fn preview(&self, engine: EngineInfo) -> AppResult<DiagnosticsBundle> {
        self.bundle(engine, false)
    }

    /// Write the bundle to a user-chosen path atomically. Returns what was written.
    pub fn export(
        &self,
        path: &Path,
        engine: EngineInfo,
        include_graphics_details: bool,
    ) -> AppResult<ExportDiagnosticsResult> {
        let bundle = self.bundle(engine, include_graphics_details)?;
        let bytes = serde_json::to_vec_pretty(&bundle)
            .map_err(|_| AppError::from(ErrorCode::DiagnosticsExportFailed))?;
        write_atomic(path, &bytes)
            .map_err(|_| AppError::from(ErrorCode::DiagnosticsExportFailed))?;
        Ok(ExportDiagnosticsResult {
            path: path.to_string_lossy().into_owned(),
            event_count: bundle.manifest.event_count,
            bytes: bytes.len() as u64,
        })
    }

    fn bundle(
        &self,
        engine: EngineInfo,
        include_graphics_details: bool,
    ) -> AppResult<DiagnosticsBundle> {
        // Query the real system configuration without holding the store lock — it can take
        // ~a second and runs on a blocking thread.
        let system = query_system_info(include_graphics_details);
        let state = self
            .inner
            .lock()
            .map_err(|_| AppError::from(ErrorCode::DiagnosticsUnavailable))?;
        let events = read_events(&state.dir.join(EVENTS_FILE))
            .map_err(|_| AppError::from(ErrorCode::DiagnosticsUnavailable))?;
        // Second validation before export: even a hand-edited log line cannot smuggle a
        // non-allowlisted `code` through, and any unknown enum variant already failed to parse.
        let events = trim_by_age(events);
        let manifest = build_manifest(engine, system, include_graphics_details, events.len());
        Ok(DiagnosticsBundle { manifest, events })
    }
}

/// Assemble the manifest from the resolved engine + the real system configuration. Kept pure so
/// it can be unit-tested without running the PowerShell query.
fn build_manifest(
    engine: EngineInfo,
    system: RawSystemInfo,
    include_graphics_details: bool,
    event_count: usize,
) -> DiagnosticsManifest {
    DiagnosticsManifest {
        schema_version: DIAGNOSTICS_SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        engine_version: engine.version,
        engine_flag_dialect: engine.flag_dialect,
        windows_version: windows_version_label(&system),
        cpu_model: system.cpu_name,
        cpu_cores: system.cpu_cores,
        memory_bytes: system.memory_bytes,
        gpus: if include_graphics_details {
            system.gpus
        } else {
            Vec::new()
        },
        exported_at: now_rfc3339(),
        event_count,
        includes_graphics_details: include_graphics_details,
    }
}

/// Real machine configuration, gathered locally through WMI on the user's explicit request.
///
/// This is the *physical* machine — the opposite of the spoofed fingerprint. It is the kind of
/// detail a support report needs (does the GPU expose D3D12? how much RAM?), and it carries
/// nothing profile- or fingerprint-shaped. The query is a fixed, read-only PowerShell command
/// with no user input and no network access; any failure degrades to empty fields.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSystemInfo {
    #[serde(default)]
    os_name: Option<String>,
    #[serde(default)]
    os_version: Option<String>,
    #[serde(default)]
    cpu_name: Option<String>,
    #[serde(default)]
    cpu_cores: Option<u32>,
    #[serde(default)]
    memory_bytes: Option<u64>,
    #[serde(default)]
    gpus: Vec<String>,
}

const PS_BASE: &str = "$os=Get-CimInstance Win32_OperatingSystem; $cpu=Get-CimInstance Win32_Processor; $cs=Get-CimInstance Win32_ComputerSystem; ";
const PS_GPU: &str = "$g=Get-CimInstance Win32_VideoController; $gpus=@($g|%{$_.Name}); ";
const PS_NO_GPU: &str = "$gpus=@(); ";
const PS_TAIL: &str = "[pscustomobject]@{osName=$os.Caption; osVersion=$os.Version; cpuName=$cpu.Name; cpuCores=$cpu.NumberOfLogicalProcessors; memoryBytes=$cs.TotalPhysicalMemory; gpus=$gpus} | ConvertTo-Json -Compress";

fn powershell_system_query(include_graphics: bool) -> String {
    let gpu = if include_graphics { PS_GPU } else { PS_NO_GPU };
    format!("{PS_BASE}{gpu}{PS_TAIL}")
}

fn query_system_info(include_graphics: bool) -> RawSystemInfo {
    // Unit tests must stay hermetic and fast — they never spawn PowerShell.
    if cfg!(test) {
        return RawSystemInfo::default();
    }
    let script = powershell_system_query(include_graphics);
    let output = match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        _ => return RawSystemInfo::default(),
    };
    serde_json::from_slice::<RawSystemInfo>(&output).unwrap_or_default()
}

fn windows_version_label(system: &RawSystemInfo) -> String {
    match (&system.os_name, &system.os_version) {
        (Some(name), Some(version)) => format!("{name} ({version})"),
        _ => format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
    }
}

// ---- File helpers -----------------------------------------------------------

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

/// Read every well-formed event, skipping corrupt lines (never fail the whole read over one).
fn read_events(path: &Path) -> std::io::Result<Vec<DiagnosticEvent>> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    Ok(contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<DiagnosticEvent>(line)
                .ok()
                .filter(|event| code_is_allowlisted(event.code.as_deref()))
        })
        .collect())
}

fn read_max_seq(dir: &Path) -> u64 {
    read_events(&dir.join(EVENTS_FILE))
        .map(|events| events.iter().map(|event| event.seq).max().unwrap_or(0))
        .unwrap_or(0)
}

/// Drop events older than [`MAX_EVENT_AGE_DAYS`]. An event whose timestamp cannot be parsed is
/// kept (we wrote it; only a hand-edited file would trip this).
fn trim_by_age(events: Vec<DiagnosticEvent>) -> Vec<DiagnosticEvent> {
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(MAX_EVENT_AGE_DAYS);
    events
        .into_iter()
        .filter(|event| {
            time::OffsetDateTime::parse(&event.at, &time::format_description::well_known::Rfc3339)
                .map(|parsed| parsed >= cutoff)
                .unwrap_or(true)
        })
        .collect()
}

/// Keep only the newest events that fit within `max_bytes`, rewriting atomically.
fn trim_to_fit(path: &Path, max_bytes: u64) -> std::io::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let mut lines: Vec<&str> = contents.lines().collect();
    let mut total: u64 = lines.iter().map(|line| line.len() as u64 + 1).sum();
    while total > max_bytes && lines.len() > 1 {
        total -= lines[0].len() as u64 + 1;
        lines.remove(0);
    }
    let kept = lines.join("\n");
    write_atomic(path, kept.as_bytes())
}

/// Write to a temporary file then rename, so a crash mid-write never leaves a truncated file.
/// Windows `rename` refuses to overwrite, so an existing destination is removed first.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&temp);
    std::fs::write(&temp, bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&temp, path)
}

/// Every `ErrorCode::as_str()` value is `[a-z][A-Za-z0-9.]*` — lowercase first char, then
/// alphanumerics and dots. A path, URL, IP or token cannot match that shape.
fn code_is_allowlisted(code: Option<&str>) -> bool {
    match code {
        None => true,
        Some(value) => {
            let mut chars = value.chars();
            match chars.next() {
                Some(first) => {
                    first.is_ascii_lowercase()
                        && chars.all(|c| c.is_ascii_alphanumeric() || c == '.')
                }
                None => false,
            }
        }
    }
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (tempfile::TempDir, DiagnosticsService) {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = DiagnosticsService::new(temp.path());
        (temp, service)
    }

    fn engine() -> EngineInfo {
        EngineInfo {
            version: Some("151.0.7922.137-1.1001".to_string()),
            flag_dialect: Some("veil-patched".to_string()),
        }
    }

    #[test]
    fn records_and_previews_a_round_trip() {
        let (_temp, service) = service();
        service.record_ok(DiagnosticComponent::Startup, DiagnosticAction::AppStarted);
        service.record_failure(
            DiagnosticComponent::Launch,
            DiagnosticAction::Launch,
            ErrorCode::LaunchRuntimeMissing,
        );

        let bundle = service.preview(engine()).expect("preview");
        assert_eq!(bundle.manifest.schema_version, DIAGNOSTICS_SCHEMA_VERSION);
        assert_eq!(
            bundle.manifest.engine_version.as_deref(),
            Some("151.0.7922.137-1.1001")
        );
        assert_eq!(
            bundle.manifest.engine_flag_dialect.as_deref(),
            Some("veil-patched")
        );
        assert_eq!(bundle.manifest.event_count, 2);
        assert_eq!(bundle.events.len(), 2);
        assert_eq!(bundle.events[0].component, DiagnosticComponent::Startup);
        assert_eq!(
            bundle.events[1].code.as_deref(),
            Some("launch.runtimeMissing")
        );
    }

    /// The privacy canary: nothing outside the allowlisted vocabulary can reach the bundle,
    /// and a hand-crafted log line carrying a path cannot be smuggled through the read path.
    #[test]
    fn no_canary_value_reaches_the_bundle() {
        let (_temp, service) = service();
        service.record_ok(DiagnosticComponent::Startup, DiagnosticAction::AppStarted);
        service.record_failure(
            DiagnosticComponent::Proxy,
            DiagnosticAction::Test,
            ErrorCode::ProxyTestFailed,
        );

        // Inject a malicious line into the on-disk log: a `component` that is a Windows path and a
        // `code` that is a URL. Neither enum variant nor code shape is allowlisted, so both must
        // be dropped on read.
        let path = _temp.path().join("diagnostics").join(EVENTS_FILE);
        append_line(
            &path,
            r#"{"schemaVersion":1,"seq":999,"at":"2026-01-01T00:00:00Z","relMs":1,"component":"C:\\Users\\bob","action":"launch","severity":"error","outcome":"failed","code":"https://evil.example"}"#,
        )
        .expect("append");

        let bundle = service.preview(engine()).expect("preview");
        let bytes = serde_json::to_vec(&bundle).expect("serialize");
        let text = String::from_utf8_lossy(&bytes);

        for canary in [
            "C:\\Users",
            "C:/Users",
            "https://evil",
            "http://",
            "secret-token",
            "password",
            "192.0.2.1",
        ] {
            assert!(
                !text.contains(canary),
                "canary {canary:?} leaked into the diagnostic bundle"
            );
        }
        assert!(bundle.events.iter().all(|e| e.seq != 999));
    }

    #[test]
    fn drops_events_older_than_the_retention_window() {
        let (_temp, service) = service();
        service.record_ok(DiagnosticComponent::Startup, DiagnosticAction::AppStarted);

        // Append a well-formed but ancient event directly.
        let path = _temp.path().join("diagnostics").join(EVENTS_FILE);
        append_line(
            &path,
            r#"{"schemaVersion":1,"seq":900,"at":"2020-01-01T00:00:00Z","relMs":1,"component":"engine","action":"install","severity":"info","outcome":"ok","code":null}"#,
        )
        .expect("append");

        let bundle = service.preview(engine()).expect("preview");
        assert!(
            bundle.events.iter().all(|e| e.seq != 900),
            "an 8-day-old event must be excluded"
        );
    }

    #[test]
    fn rotation_drops_oldest_events_to_fit_the_budget() {
        let (_temp, service) = service();
        for _ in 0..50 {
            service.record_ok(DiagnosticComponent::Engine, DiagnosticAction::Install);
        }
        let path = _temp.path().join("diagnostics").join(EVENTS_FILE);
        let before = read_events(&path).expect("read");
        assert_eq!(before.len(), 50);

        // Rotate to a tiny budget: only the newest events survive, in order.
        trim_to_fit(&path, 500).expect("trim");
        let kept = read_events(&path).expect("read");
        assert!(kept.len() < 50);
        assert!(std::fs::metadata(&path).expect("metadata").len() <= 500);

        let max_seq = before.iter().map(|e| e.seq).max().unwrap();
        assert_eq!(kept.iter().map(|e| e.seq).max().unwrap(), max_seq);
        assert!(kept.iter().all(|e| e.seq > max_seq - kept.len() as u64));
    }

    #[test]
    fn export_matches_preview_and_writes_atomically() {
        let (_temp, service) = service();
        service.record_failure(
            DiagnosticComponent::Vault,
            DiagnosticAction::Unlock,
            ErrorCode::StorageDecryptionFailed,
        );

        let preview = service.preview(engine()).expect("preview");
        let out = _temp.path().join("export.json");
        let result = service.export(&out, engine(), false).expect("export");
        assert_eq!(result.event_count, preview.manifest.event_count);
        assert!(out.exists());

        let written: DiagnosticsBundle =
            serde_json::from_slice(&std::fs::read(&out).expect("read")).expect("parse");
        assert_eq!(written.events.len(), preview.events.len());
        assert_eq!(
            written.events[0].code.as_deref(),
            Some("storage.decryptionFailed")
        );
    }

    #[test]
    fn tolerates_a_corrupt_line_without_failing() {
        let (_temp, service) = service();
        let path = _temp.path().join("diagnostics").join(EVENTS_FILE);
        append_line(&path, "{ this is not json").expect("append");
        service.record_ok(DiagnosticComponent::Startup, DiagnosticAction::AppStarted);

        let bundle = service.preview(engine()).expect("preview");
        assert_eq!(bundle.events.len(), 1);
    }

    #[test]
    fn unknown_enum_variants_are_rejected_on_read() {
        let (_temp, service) = service();
        let path = _temp.path().join("diagnostics").join(EVENTS_FILE);
        // `component: "network"` is not an allowlisted variant, so the line must be dropped.
        append_line(
            &path,
            r#"{"schemaVersion":1,"seq":1,"at":"2026-01-01T00:00:00Z","relMs":1,"component":"network","action":"install","severity":"info","outcome":"ok","code":null}"#,
        )
        .expect("append");

        let bundle = service.preview(engine()).expect("preview");
        assert!(bundle.events.is_empty());
    }

    #[test]
    fn manifest_carries_system_config_and_gates_graphics() {
        let system = RawSystemInfo {
            os_name: Some("Microsoft Windows 11 Pro".to_string()),
            os_version: Some("10.0.26100".to_string()),
            cpu_name: Some("Intel(R) Core(TM) i5-12400".to_string()),
            cpu_cores: Some(12),
            memory_bytes: Some(17_179_869_184),
            gpus: vec!["NVIDIA GeForce RTX 50-series card".to_string()],
        };

        let base = build_manifest(engine(), system.clone(), false, 3);
        assert_eq!(
            base.cpu_model.as_deref(),
            Some("Intel(R) Core(TM) i5-12400")
        );
        assert_eq!(base.cpu_cores, Some(12));
        assert_eq!(base.memory_bytes, Some(17_179_869_184));
        assert_eq!(
            base.windows_version,
            "Microsoft Windows 11 Pro (10.0.26100)"
        );
        assert!(
            base.gpus.is_empty(),
            "graphics must not leak without opt-in"
        );
        assert!(!base.includes_graphics_details);

        let with_gpu = build_manifest(engine(), system, true, 3);
        assert_eq!(
            with_gpu.gpus,
            vec!["NVIDIA GeForce RTX 50-series card".to_string()]
        );
        assert!(with_gpu.includes_graphics_details);
    }

    #[test]
    fn windows_version_falls_back_when_the_query_fails() {
        let label = windows_version_label(&RawSystemInfo::default());
        assert_eq!(
            label,
            format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH)
        );
    }
}
