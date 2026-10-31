//! Manual real-engine verification against the configured engine (`Veil-Chromium`, Veil's own
//! build of ungoogled-chromium).
//!
//! Both tests are ignored by default: they perform explicit network access and download the full
//! Windows engine zip (~195 MB). Run only as a user-requested install verification:
//!
//! ```text
//! cargo test -p veil-runtime --test ungoogled_real -- --ignored --nocapture
//! ```

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::{env, fs, io::Read, path::Path};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use veil_runtime::{
    fetch_releases, install_pipeline, select_candidates, EngineManifest, FlagDialect, Progress,
    RuntimeCandidate,
};

#[test]
#[ignore = "requires network and downloads the full Veil-Chromium Windows zip"]
fn installs_current_engine_release_from_github() {
    let manifest = EngineManifest::default();
    assert_eq!(manifest.name, "Veil-Chromium");
    assert_eq!(manifest.exe_path_in_zip, "chrome.exe");
    // Since 2026-08-23 the fork publishes a build that carries the patch set, so the manifest
    // claims `veil-patched`. Before that it shipped zero fingerprint patches and this asserted
    // `StockChromium`.
    assert_eq!(manifest.flag_dialect, FlagDialect::VeilPatched);

    let releases = fetch_releases(&manifest).expect("fetch releases");
    let candidates = select_candidates(&releases, &manifest);
    assert!(
        !candidates.is_empty(),
        "expected at least one compatible Veil-Chromium Windows x64 release"
    );

    // `select_candidates` must hand back the newest version first regardless of API order.
    let newest = &candidates[0];
    for other in &candidates[1..] {
        assert!(
            version_key(&newest.version) >= version_key(&other.version),
            "{} should not rank below {}",
            newest.version,
            other.version
        );
    }

    let candidate = newest;
    eprintln!(
        "selected {} ({}, {} bytes)",
        candidate.version,
        candidate.asset_name,
        candidate.size_bytes.unwrap_or(0)
    );
    assert!(candidate.asset_name.starts_with("ungoogled-chromium_"));
    assert!(candidate.asset_name.ends_with("_windows_x64.zip"));
    // The checksum comes from the GitHub API asset digest — there is no checksum file to fetch.
    assert_eq!(candidate.sha256.len(), 64);
    assert!(candidate.sha256.chars().all(|c| c.is_ascii_hexdigit()));

    let dest = TempDir::new().expect("runtime destination");
    let cancel = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let events_clone = events.clone();

    let installed = install_pipeline(candidate, dest.path(), &manifest, cancel, move |progress| {
        log_progress(&progress);
        events_clone.lock().expect("events lock").push(progress);
    })
    .expect("install ungoogled-chromium");

    assert_eq!(installed.version, candidate.version);
    assert_eq!(installed.channel, "stable");
    // The zip nests everything under one top-level directory; the pipeline must collapse it so
    // `chrome.exe` lands directly in the version directory.
    assert!(installed.executable_path.ends_with("chrome.exe"));
    assert!(installed.executable_path.exists());
    assert_eq!(
        installed.executable_path.parent(),
        Some(dest.path().join(&candidate.version).as_path()),
        "chrome.exe must sit at the root of the version directory"
    );

    let events = events.lock().expect("events lock");
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Downloading { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Verifying)));
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Extracting { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Finalizing)));
    assert_eq!(events.last(), Some(&Progress::Completed));
}

#[test]
#[ignore = "requires VEIL_ENGINE_ZIP pointing at a real ungoogled-chromium Windows zip"]
fn installs_local_engine_zip() {
    let zip_path = env::var("VEIL_ENGINE_ZIP").expect("set VEIL_ENGINE_ZIP to the downloaded zip");
    let zip_path = Path::new(&zip_path);
    assert!(zip_path.exists(), "zip must exist: {}", zip_path.display());

    let manifest = EngineManifest::default();
    let candidate = RuntimeCandidate {
        version: "local-real-engine".to_string(),
        asset_name: zip_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "engine.zip".to_string()),
        download_url: file_url(zip_path),
        sha256: sha256_file(zip_path),
        size_bytes: fs::metadata(zip_path).ok().map(|m| m.len()),
        published_at: None,
    };
    let dest = TempDir::new().expect("runtime destination");
    let cancel = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let events_clone = events.clone();

    let installed = install_pipeline(
        &candidate,
        dest.path(),
        &manifest,
        cancel,
        move |progress| {
            log_progress(&progress);
            events_clone.lock().expect("events lock").push(progress);
        },
    )
    .expect("install local engine zip");

    assert_eq!(installed.channel, "stable");
    assert!(installed.executable_path.ends_with("chrome.exe"));
    assert!(installed.executable_path.exists());

    let events = events.lock().expect("events lock");
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Verifying)));
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Extracting { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, Progress::Completed)));
}

/// Same component-wise ordering the provider uses, so this test asserts real ordering rather than
/// a string compare (`151.0.7922.75` must rank below `151.0.7922.108`).
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-'])
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn sha256_file(path: &Path) -> String {
    let mut file = fs::File::open(path).expect("open zip");
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).expect("read zip");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace('\\', "/"))
}

fn log_progress(progress: &Progress) {
    match progress {
        Progress::Extracting {
            entries_done,
            entries_total,
        } => {
            if *entries_done == 0 || *entries_done == *entries_total || entries_done % 200 == 0 {
                eprintln!("progress: {progress:?}");
            }
        }
        _ => eprintln!("progress: {progress:?}"),
    }
}
