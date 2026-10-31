//! Dev-only launcher for the detection baseline harness (see
//! `tools/detection-baseline/README.md`).
//!
//! Both tests are `#[ignore]`d: they launch a real browser, and one of them downloads ~188 MB.
//! Nothing here is reachable from the app — there is no Tauri command, no service, and no CI job
//! that runs it. It exists so a maintainer can take a snapshot on demand.
//!
//! ```text
//! # one-time, or after an engine bump (explicit network access)
//! cargo test -p veil-runtime --test detection_baseline -- --ignored --exact \
//!     installs_current_engine_into_the_baseline_runtime_root --nocapture
//!
//! # take a snapshot (no network: the fixture pages are served from loopback)
//! cargo test -p veil-runtime --test detection_baseline -- --ignored --exact \
//!     captures_detection_baseline --nocapture
//! ```
//!
//! Division of labour, kept deliberate so Stage 4 can move the harness to `Veil-Chromium`:
//!
//! - **This file** owns the *launch*, and only the launch. It resolves the engine from Veil's own
//!   runtime registry, builds the argument list with `veil_core::to_engine_args` gated on the
//!   manifest's flag dialect, and spawns through `veil_bridge` — the same path
//!   `commands::profiles` uses. Nothing about the fingerprint surface is decided here.
//! - **`tools/detection-baseline`** owns the *measurement*: it attaches to the debug port and
//!   reads page state over CDP. It is launcher-agnostic, so relocating it does not require
//!   rewriting it.
//!
//! The debug port is obtained exactly the way the AI automation path obtains it
//! (`services::ai_service::AiService::allocate_debug_port`): bind `127.0.0.1:0`, take the port the
//! OS assigned, drop the listener. Same mechanism, not a second one — that service lives in the
//! app crate behind a private module and needs a database to construct, so the strategy is
//! mirrored here rather than imported.
//!
//! CDP is used to **read** only. `Emulation.*`, `Network.setUserAgentOverride` and
//! `Page.addScriptToEvaluateOnNewDocument` are forbidden project-wide; the capture tool enforces
//! that with a method allowlist.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use veil_bridge::{build_engine_args, launch, LaunchRequest};
use veil_core::{to_engine_args, Fingerprint};
use veil_runtime::{
    fetch_releases, install_pipeline, select_candidates, EngineManifest, Progress,
    RuntimeCandidate, RuntimeRegistryFile,
};

/// Metadata the capture tool records verbatim at the top of the snapshot, so the JSON says which
/// engine, which dialect, and which argument list produced the numbers underneath it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureMeta {
    snapshot_version: u32,
    engine: EngineMeta,
    #[serde(rename = "case")]
    case_meta: CaseMeta,
    launch: LaunchMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineMeta {
    name: String,
    version: String,
    flag_dialect: String,
    executable_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseMeta {
    name: String,
    /// The profile's OS. Carried into the snapshot because `--fingerprint-platform` derives from it
    /// and assertion 6 needs to know which platform the persona claims before it can ask whether the
    /// presented machine could exist.
    os: String,
    /// The persona's declared hardware, verbatim from the case file. Documentation plus assertion 6's
    /// expectation; it reaches no switch. `null` for a case that declares none.
    machine: Option<serde_json::Value>,
    fingerprint: Fingerprint,
    proxy_configured: bool,
}

/// The *complete* command line the bridge built, with the two values that legitimately change
/// between runs replaced by placeholders. Recording the real argv is the point: it makes "which
/// switches produced these values" part of the artifact instead of tribal knowledge.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchMeta {
    argv: Vec<String>,
    env: BTreeMap<String, String>,
    /// Switches injected by `VEIL_BASELINE_EXTRA_ARGS` rather than produced by
    /// `to_engine_args`. Recorded separately and never merged into `argv`'s story: a reader who
    /// finds `--fingerprint=` in `argv` must be able to tell whether the dialect emitted it (it
    /// cannot, under `stock-chromium`) or whether the measurement forced it.
    injected_args: Vec<String>,
}

#[test]
#[ignore = "requires network: downloads the full ungoogled-chromium Windows zip (~188 MB)"]
fn installs_current_engine_into_the_baseline_runtime_root() {
    let manifest = EngineManifest::default();
    let candidate = match env::var("VEIL_ENGINE_ZIP") {
        // Lets a maintainer install a zip already on disk without re-downloading 188 MB. The
        // hash is still computed and verified by the pipeline, and it is checked against the
        // GitHub asset digest by hand (recorded in tools/detection-baseline/README.md).
        Ok(path) => local_zip_candidate(Path::new(&path)),
        Err(_) => newest_release_candidate(&manifest),
    };
    eprintln!(
        "installing {} ({}, {} bytes)",
        candidate.version,
        candidate.asset_name,
        candidate.size_bytes.unwrap_or(0)
    );

    let versions_root = versions_root();
    let installed = install_pipeline(
        &candidate,
        &versions_root,
        &manifest,
        Arc::new(AtomicBool::new(false)),
        |progress| {
            if !matches!(progress, Progress::Extracting { .. }) {
                eprintln!("progress: {progress:?}");
            }
        },
    )
    .expect("install engine");

    assert!(installed.executable_path.exists());
    let registry_path = registry_path();
    let mut registry = RuntimeRegistryFile::load(&registry_path).expect("load registry");
    registry.upsert(installed.clone(), true);
    registry.save(&registry_path).expect("save registry");
    eprintln!(
        "installed {} -> {}\nregistry: {}",
        installed.version,
        installed.executable_path.display(),
        registry_path.display()
    );
}

#[test]
#[ignore = "launches the installed engine and runs the Node capture harness"]
fn captures_detection_baseline() {
    let manifest = EngineManifest::default();
    let engine = resolve_engine(&manifest);
    let case_name = env::var("VEIL_BASELINE_CASE").unwrap_or_else(|_| "default".to_string());
    let (case_os, fingerprint, case_aside) = load_case(&case_name);
    veil_core::validate(&fingerprint, case_os).expect("case fingerprint must be coherent");

    // The dialect gate is the whole point: on `stock-chromium` most of the case's fields emit
    // nothing, and the snapshot must be taken with exactly the switches Veil would really pass.
    // It comes off the *engine being launched*, the same source `commands::profiles` now reads
    // (5.5c) — a manifest-global dialect would label a snapshot with capabilities the binary under
    // test may not have. The OS comes with the case rather than out of the fingerprint, because that
    // is where a real profile keeps it — `--fingerprint-platform` is derived from `Profile::os` (5.5b).
    let dialect = engine.effective_dialect();
    let options = to_engine_args(&fingerprint, case_os, dialect);
    let debug_port = allocate_debug_port();
    let proxy = env::var("VEIL_BASELINE_PROXY")
        .ok()
        .filter(|p| !p.is_empty());

    let mut extra_args = vec![
        format!("--remote-debugging-port={debug_port}"),
        "--remote-debugging-address=127.0.0.1".to_string(),
    ];
    if let Some(proxy) = &proxy {
        extra_args.push(format!("--proxy-server={proxy}"));
    }
    extra_args.extend(options.args.clone());
    // Switches the current dialect cannot emit but a patch under test reads — see `injected_args`.
    // They go *after* the dialect's own arguments so that a duplicate resolves to the injected
    // value: Chromium's command line keeps the last occurrence of a switch.
    let injected_args = injected_args();
    extra_args.extend(injected_args.clone());
    // Veil always appends a startup URL; the harness navigates its own tab, so this one stays blank.
    extra_args.push("about:blank".to_string());

    let user_data_dir = tempfile::TempDir::new().expect("user-data-dir");
    let request = LaunchRequest {
        executable_path: engine.executable_path.clone(),
        user_data_dir: user_data_dir.path().to_path_buf(),
        profile_id: format!("detection-baseline-{case_name}"),
        extra_args,
        env: options.env.clone(),
    };
    let argv = redact_argv(
        &build_engine_args(&request),
        debug_port,
        user_data_dir.path(),
        proxy.as_deref(),
    );

    let meta = CaptureMeta {
        snapshot_version: 1,
        engine: EngineMeta {
            name: engine
                .engine_name
                .clone()
                .unwrap_or_else(|| manifest.name.clone()),
            version: engine.version.clone(),
            flag_dialect: dialect_label(dialect),
            executable_sha256: sha256_file(&engine.executable_path),
        },
        case_meta: CaseMeta {
            name: case_name.clone(),
            os: case_os.as_str().to_string(),
            machine: case_aside.machine.clone(),
            fingerprint,
            proxy_configured: proxy.is_some(),
        },
        launch: LaunchMeta {
            argv,
            env: options.env.iter().cloned().collect(),
            injected_args: injected_args.clone(),
        },
    };

    let (process, result) = launch(&request).expect("launch engine");
    let mut guard = EngineGuard(process);
    eprintln!("engine pid {} on port {debug_port}", result.process_id);
    wait_for_cdp(debug_port).expect("engine did not open its debug port");

    let out_dir = snapshots_dir().join(&engine.version);
    fs::create_dir_all(&out_dir).expect("snapshot dir");
    let out_path = out_dir.join(format!("{}-{case_name}.json", dialect_label(dialect)));
    let meta_path = out_dir.join(format!(".meta-{case_name}.json"));
    fs::write(
        &meta_path,
        serde_json::to_vec_pretty(&meta).expect("serialize meta"),
    )
    .expect("write meta");

    let status = Command::new(node_command())
        .current_dir(repo_root())
        .arg("tools/detection-baseline/capture.mjs")
        .arg("--cdp")
        .arg(format!("http://127.0.0.1:{debug_port}"))
        .arg("--meta")
        .arg(&meta_path)
        .arg("--out")
        .arg(&out_path)
        .status()
        .expect("run the Node capture harness (Node >= 22.4 must be on PATH)");
    let _ = fs::remove_file(&meta_path);
    let _ = guard.0.stop();
    assert!(status.success(), "capture harness failed: {status}");

    let snapshot = fs::read_to_string(&out_path).expect("snapshot written");
    let parsed: serde_json::Value = serde_json::from_str(&snapshot).expect("snapshot is JSON");
    for key in ["engine", "case", "launch", "realms", "assertions"] {
        assert!(parsed.get(key).is_some(), "snapshot is missing `{key}`");
    }
    eprintln!("snapshot: {}", out_path.display());
}

/// The committed cases are data, and data with no test behind it drifts.
///
/// `captures_detection_baseline` already loads and validates them, but it is `#[ignore]`d and needs an
/// installed engine, so a malformed case file would sit in the tree until someone with a build noticed.
/// This runs in plain `cargo test --workspace`.
///
/// The last two assertions are assertion 6's cheapest rules, applied to the *declaration* rather than to
/// a capture: a persona whose declared window does not fit its declared panel describes a machine that
/// cannot exist, and no amount of engine work will fix that. Catching it here costs milliseconds; catching
/// it in a snapshot costs a 28-minute build and a capture.
///
/// Since T06 the case file states the panel **twice**, and that is the point rather than a redundancy:
/// `machine.display` is the persona's declaration and `fingerprint.panel_*` is what Veil actually sends,
/// so `display-matches-the-declared-panel` compares two independent statements. If the case file were
/// allowed to disagree with itself, the rule would be scoring the engine against a claim Veil never made.
///
/// `persona-no-display` is `persona` with the display block removed and no `machine` at all: the control
/// that shows `960` is inert without its three switches. It declares no panel, so the paired assertions
/// below skip it — what it has to satisfy is `validate`, which is where "a panel needs a seed" and the
/// work-area fit live.
#[test]
fn committed_cases_are_loadable_and_coherent() {
    for case in [
        "default",
        "persona",
        "persona-no-display",
        "persona-android",
        "t08-windows",
    ] {
        let (os, fingerprint, aside) = load_case(case);
        veil_core::validate(&fingerprint, os)
            .unwrap_or_else(|err| panic!("case {case} is not coherent: {err:?}"));
        assert!(
            !aside.os.is_empty(),
            "case {case} must declare `os`: assertion 6 has no claim to check without it"
        );
        assert_eq!(
            os.as_str(),
            aside.os,
            "case {case} declares an `os` that `ProfileOs` does not recognise, so it silently became \
             the default"
        );

        let Some(display) = aside
            .machine
            .as_ref()
            .and_then(|machine| machine.get("display"))
        else {
            continue;
        };
        let dimension = |key: &str| display.get(key).and_then(serde_json::Value::as_u64);
        let (width, height) = (
            dimension("width").expect("declared display needs a width"),
            dimension("height").expect("declared display needs a height"),
        );
        // The two statements of the panel have to agree, or the rule they feed is vacuous.
        assert_eq!(
            (
                u64::from(fingerprint.panel_width),
                u64::from(fingerprint.panel_height)
            ),
            (width, height),
            "case {case}: `machine.display` declares {width}×{height} but the fingerprint sends \
             {}×{}, so `display-matches-the-declared-panel` would score the engine against a panel \
             Veil never asked for",
            fingerprint.panel_width,
            fingerprint.panel_height
        );
        let declared_ratio = display
            .get("devicePixelRatio")
            .and_then(serde_json::Value::as_f64)
            .expect("declared display needs a devicePixelRatio");
        assert!(
            (fingerprint.device_pixel_ratio - declared_ratio).abs() < f64::EPSILON,
            "case {case}: `machine.display` declares {declared_ratio}× and the fingerprint sends {}×",
            fingerprint.device_pixel_ratio
        );
        // The window fits the *work area*, not merely the panel — the stronger of the two, and the
        // one `window-fits-the-work-area` scores now that `960` drives `screen.avail*`.
        let (work_width, work_height) = veil_core::derived_work_area(&fingerprint, os)
            .unwrap_or_else(|| panic!("case {case}: a declared panel must leave a work area"));
        assert!(
            fingerprint.screen_width <= work_width && fingerprint.screen_height <= work_height,
            "case {case}: a {}×{} window does not fit the {work_width}×{work_height} work area a \
             {width}×{height} {} panel leaves",
            fingerprint.screen_width,
            fingerprint.screen_height,
            os.as_str()
        );
    }
}

/// Extra engine switches for the measurement only, from `VEIL_BASELINE_EXTRA_ARGS`.
///
/// Stage 5 needs this and cannot proceed without it. Eleven of the sixteen Tier 1 patches are
/// driven by `--fingerprint=<seed>`, `018` by `--timezone=`, `005` by
/// `--fingerprint-hardware-concurrency=`; under `stock-chromium` `to_engine_args` correctly emits
/// **none** of them, so a freshly built patch could not be measured at all. The alternatives are
/// both worse: flipping `engine.toml` to `veil-patched` would emit six switches Tier 1 does not
/// implement, and adding the switches to the committed case
/// files would make them describe something other than a Veil profile.
///
/// This is measurement scaffolding, not product behaviour. `to_engine_args` is untouched, the
/// dialect gate still decides what Veil itself emits, and every injected switch is recorded in the
/// snapshot's `launch.injectedArgs` so no reader can mistake one for a dialect emission.
///
/// Splitting is on whitespace, so a switch value containing a space cannot be passed. None of the
/// Tier 1 switches take one; if that ever changes, take a JSON array here rather than inventing a
/// quoting rule.
fn injected_args() -> Vec<String> {
    env::var("VEIL_BASELINE_EXTRA_ARGS")
        .ok()
        .into_iter()
        .flat_map(|raw| {
            raw.split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Kills the engine even when an assertion panics, so a failed run does not leave a browser and a
/// listening debug port behind.
struct EngineGuard(veil_bridge::EngineProcess);

impl Drop for EngineGuard {
    fn drop(&mut self) {
        let _ = self.0.stop();
    }
}

/// Same strategy as `services::ai_service::AiService::allocate_debug_port`: let the OS pick a free
/// loopback port, then release it so the engine can bind it.
fn allocate_debug_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// Poll the DevTools HTTP endpoint until the engine answers. Plain HTTP, no WebSocket: this only
/// establishes that the browser is up before handing the port to the capture tool.
fn wait_for_cdp(port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|err| err.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut last = String::from("not attempted");
    while Instant::now() < deadline {
        match client.get(&url).send() {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => last = format!("status {}", response.status()),
            Err(err) => last = err.to_string(),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(last)
}

/// Replace the three argv values that legitimately differ between runs, so two consecutive
/// snapshots of the same engine diff clean: the ephemeral debug port, the throwaway user-data
/// directory, and the proxy (which may carry credentials — the scheme is kept because it changes
/// how the WebRTC result should be read).
fn redact_argv(
    argv: &[String],
    debug_port: u16,
    user_data_dir: &Path,
    proxy: Option<&str>,
) -> Vec<String> {
    let port_switch = format!("--remote-debugging-port={debug_port}");
    let dir = user_data_dir.to_string_lossy().replace('\\', "/");
    argv.iter()
        .map(|arg| {
            if let Some(proxy) = proxy {
                if arg == &format!("--proxy-server={proxy}") {
                    let scheme = proxy.split("://").next().unwrap_or("proxy");
                    return format!("--proxy-server={scheme}://<redacted>");
                }
            }
            if arg == &port_switch {
                // Matched as a whole switch rather than by substring: the port digits could occur
                // inside a user agent or a path, and a snapshot that quietly mangles an argument is
                // worse than one that churns.
                return "--remote-debugging-port=<ephemeral-port>".to_string();
            }
            arg.replace(&dir, "<user-data-dir>")
        })
        .collect()
}

fn dialect_label(dialect: veil_runtime::FlagDialect) -> String {
    serde_json::to_value(dialect)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Resolve the engine the same way the app does: whatever Veil's runtime registry calls current,
/// **including the dialect recorded on that entry** (5.5c). `VEIL_ENGINE_EXE` is an escape hatch for
/// pointing the harness at a build that is not installed through Veil (a locally built
/// `Veil-Chromium`, for instance).
///
/// On the escape-hatch path there is no registry entry to read a dialect off, so one has to be
/// supplied. It defaults to the manifest's, because a maintainer who points the harness at an
/// unregistered `chrome.exe` is asserting *which* engine it is and `engine.toml` is that assertion —
/// and `VEIL_ENGINE_DIALECT` overrides it, so measuring a stock engine (or a patched one while the
/// manifest still says stock) never requires editing this file. That default is the opposite of the
/// app's, deliberately: the app must not trust a manifest for an engine it did not install, while
/// here the manifest and the engine under test are the same claim.
fn resolve_engine(manifest: &EngineManifest) -> veil_runtime::InstalledRuntime {
    if let Ok(path) = env::var("VEIL_ENGINE_EXE") {
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "VEIL_ENGINE_EXE does not exist: {}",
            path.display()
        );
        return veil_runtime::InstalledRuntime {
            version: env::var("VEIL_ENGINE_VERSION").unwrap_or_else(|_| "unregistered".to_string()),
            channel: "stable".to_string(),
            executable_path: path,
            installed_at: String::new(),
            sha256: String::new(),
            size_bytes: None,
            flag_dialect: Some(dialect_override().unwrap_or(manifest.flag_dialect)),
            engine_name: Some(manifest.name.clone()),
            github_owner: Some(manifest.github_owner.clone()),
            github_repo: Some(manifest.github_repo.clone()),
        };
    }
    let registry_path = registry_path();
    let registry = RuntimeRegistryFile::load(&registry_path).expect("load runtime registry");
    let current = registry.current().unwrap_or_else(|| {
        panic!(
            "no engine registered in {}. Run the `installs_current_engine_into_the_baseline_runtime_root` \
             test first, or set VEIL_ENGINE_EXE.",
            registry_path.display()
        )
    });
    assert!(
        current.executable_path.exists(),
        "registered engine is missing: {}",
        current.executable_path.display()
    );
    let mut resolved = current.clone();
    // An override still applies to a registered engine: re-measuring an entry installed before the
    // dialect was recorded would otherwise be stuck at `stock-chromium` with no way to say otherwise.
    if let Some(dialect) = dialect_override() {
        resolved.flag_dialect = Some(dialect);
    }
    resolved
}

/// `VEIL_ENGINE_DIALECT=stock-chromium|veil-patched`, parsed through the same serde representation
/// `engine.toml` uses so the spelling cannot drift. An unset variable is `None`; an unrecognized value
/// is a panic rather than a silent fallback — a typo here would mislabel the snapshot.
fn dialect_override() -> Option<veil_runtime::FlagDialect> {
    let raw = env::var("VEIL_ENGINE_DIALECT").ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    Some(
        serde_json::from_value(serde_json::Value::String(raw.to_string())).unwrap_or_else(|_| {
            panic!("VEIL_ENGINE_DIALECT must be `stock-chromium` or `veil-patched`, got {raw:?}")
        }),
    )
}

/// The fingerprint a case launches with, committed as JSON next to the harness so a snapshot is
/// reproducible from the repo alone. `default` is the true floor (an all-empty fingerprint emits no
/// switches at all); `persona` sets the four fields a stock engine does accept, which is what makes
/// the UA-CH and `--window-size` floors visible in the numbers.
/// A case describes a *profile's* launch-relevant configuration, which is an OS plus a fingerprint —
/// not a fingerprint alone. `--fingerprint-platform` is derived from `Profile::os`, and putting a copy
/// of the platform inside `Fingerprint` would create a second source of truth for it (5.5b). The two
/// are read out of the same file: `Fingerprint` ignores the extra `os` key, and `CaseAside` ignores
/// everything else.
///
/// `machine` is the third thing in the file and it reaches **no** switch: it is the persona's declared
/// hardware, carried into the snapshot so assertion 6 can ask whether the engine delivered it and a
/// reader can tell whether the persona was realistic in the first place. Nothing in `to_engine_args`
/// sees it — a declaration that silently became a launch argument would be the "switch with no patch
/// behind it" bug wearing a new hat.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CaseAside {
    os: String,
    machine: Option<serde_json::Value>,
}

fn load_case(case: &str) -> (veil_core::ProfileOs, Fingerprint, CaseAside) {
    let path = harness_dir().join("cases").join(format!("{case}.json"));
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read case {}: {err}", path.display()));
    let fingerprint: Fingerprint = serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse case {}: {err}", path.display()));
    let aside: CaseAside = serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse case os {}: {err}", path.display()));
    (
        veil_core::ProfileOs::from_str_or_default(&aside.os),
        fingerprint,
        aside,
    )
}

fn newest_release_candidate(manifest: &EngineManifest) -> RuntimeCandidate {
    let releases = fetch_releases(manifest).expect("fetch releases");
    let candidates = select_candidates(&releases, manifest);
    candidates
        .into_iter()
        .next()
        .expect("no compatible engine release found")
}

fn local_zip_candidate(zip: &Path) -> RuntimeCandidate {
    assert!(zip.exists(), "zip must exist: {}", zip.display());
    RuntimeCandidate {
        version: env::var("VEIL_ENGINE_VERSION").expect(
            "set VEIL_ENGINE_VERSION alongside VEIL_ENGINE_ZIP so the install is labelled with the \
             real engine tag",
        ),
        asset_name: zip.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        download_url: format!("file://{}", zip.to_string_lossy().replace('\\', "/")),
        sha256: sha256_file(zip),
        size_bytes: fs::metadata(zip).ok().map(|meta| meta.len()),
        published_at: None,
    }
}

fn sha256_file(path: &Path) -> String {
    let mut file =
        fs::File::open(path).unwrap_or_else(|err| panic!("open {}: {err}", path.display()));
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).expect("read file");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn harness_dir() -> PathBuf {
    repo_root().join("tools").join("detection-baseline")
}

fn snapshots_dir() -> PathBuf {
    harness_dir().join("snapshots")
}

/// Where the engine is installed and registered. Defaults to the directory layout the app uses
/// (`<app-local-data>/Veil/runtime/{registry.json,versions}`), so an engine installed by the
/// harness is the same engine the Browser page sees.
fn runtime_root() -> PathBuf {
    if let Ok(root) = env::var("VEIL_BASELINE_RUNTIME_ROOT") {
        return PathBuf::from(root);
    }
    match env::var("LOCALAPPDATA") {
        Ok(local) => PathBuf::from(local).join("Veil").join("runtime"),
        Err(_) => repo_root().join("target").join("baseline-runtime"),
    }
}

fn registry_path() -> PathBuf {
    runtime_root().join("registry.json")
}

fn versions_root() -> PathBuf {
    runtime_root().join("versions")
}

/// `node` on PATH, overridable for machines with several Node versions installed.
fn node_command() -> String {
    env::var("VEIL_BASELINE_NODE").unwrap_or_else(|_| "node".to_string())
}
