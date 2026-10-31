//! End-to-end pipeline tests, fully offline (file:// URLs only).

use std::fs;
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tempfile::TempDir;
use veil_runtime::{
    install_pipeline, sha256_hex, EngineManifest, PipelineError, Progress, RuntimeCandidate,
};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn build_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::<u8>::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default();
        for (name, data) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

fn manifest() -> EngineManifest {
    use std::str::FromStr;
    EngineManifest::from_str(
        r#"
        [engine]
        name = "Test Engine"
        github_owner = "example"
        github_repo = "engine"
        version_regex = "^v?([0-9]+(?:\\.[0-9]+)*)$"
        asset_regex = "^engine-.*windows.*x64.*\\.zip$"
        exe_path_in_zip = "engine.exe"
        checksum_source = "release_body"
        channel = "free"
        "#,
    )
    .unwrap()
}

fn candidate(zip_bytes: &[u8], zip_path_url: String) -> RuntimeCandidate {
    RuntimeCandidate {
        version: "120".to_string(),
        asset_name: "engine-windows-x64.zip".to_string(),
        download_url: zip_path_url,
        sha256: sha256_hex(zip_bytes),
        size_bytes: Some(zip_bytes.len() as u64),
        published_at: None,
    }
}

fn write_zip_to_disk(dir: &TempDir, name: &str, bytes: &[u8]) -> String {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap();
    format!("file://{}", path.to_string_lossy().replace('\\', "/"))
}

#[test]
fn happy_path_installs_engine_and_emits_all_stages() {
    let zip_bytes = build_zip(&[("engine.exe", b"MZ-fake-exe")]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();
    let manifest = manifest();
    let cancel = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let events_clone = events.clone();

    let installed = install_pipeline(
        &candidate,
        dest.path(),
        &manifest,
        cancel,
        move |progress| events_clone.lock().unwrap().push(progress),
    )
    .expect("install");

    assert_eq!(installed.version, "120");
    assert_eq!(installed.channel, "free");
    assert!(
        installed.executable_path.ends_with("120/engine.exe")
            || installed.executable_path.ends_with("120\\engine.exe")
    );
    assert!(
        installed.executable_path.exists(),
        "executable must exist on disk"
    );

    let kinds: Vec<&'static str> = events
        .lock()
        .unwrap()
        .iter()
        .map(|event| match event {
            Progress::Downloading { .. } => "down",
            Progress::Verifying => "verify",
            Progress::Extracting { .. } => "extract",
            Progress::Finalizing => "final",
            Progress::Completed => "done",
        })
        .collect();
    assert!(kinds.contains(&"down"));
    assert!(kinds.contains(&"verify"));
    assert!(kinds.contains(&"extract"));
    assert!(kinds.contains(&"final"));
    assert_eq!(kinds.last().copied(), Some("done"));
}

#[test]
fn checksum_mismatch_fails_and_leaves_dest_untouched() {
    let zip_bytes = build_zip(&[("engine.exe", b"MZ")]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let mut candidate = candidate(&zip_bytes, url);
    candidate.sha256 = "0".repeat(64); // wrong on purpose
    let dest = TempDir::new().unwrap();
    let manifest = manifest();
    let cancel = Arc::new(AtomicBool::new(false));

    let result = install_pipeline(&candidate, dest.path(), &manifest, cancel, |_| {});
    assert!(matches!(
        result,
        Err(PipelineError::Install(
            veil_runtime::InstallError::ChecksumMismatch
        ))
    ));
    assert!(
        !dest.path().join("120").exists(),
        "no version dir must be created on mismatch"
    );
}

#[test]
fn missing_executable_in_zip_returns_executable_missing() {
    // Zip has no engine.exe — only a stray text file.
    let zip_bytes = build_zip(&[("readme.txt", b"hello")]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();
    let manifest = manifest();
    let cancel = Arc::new(AtomicBool::new(false));

    let result = install_pipeline(&candidate, dest.path(), &manifest, cancel, |_| {});
    assert!(matches!(result, Err(PipelineError::ExecutableMissing)));
    assert!(!dest.path().join("120").exists());
}

#[test]
fn cancel_before_finalize_leaves_dest_untouched() {
    let zip_bytes = build_zip(&[
        ("engine.exe", &vec![0u8; 4096]),
        ("a.txt", b"a"),
        ("b.txt", b"b"),
    ]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();
    let manifest = manifest();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();

    // Trip cancel as soon as we see the first Extracting event.
    let result = install_pipeline(
        &candidate,
        dest.path(),
        &manifest,
        cancel,
        move |progress| {
            if matches!(progress, Progress::Extracting { .. }) {
                cancel_clone.store(true, Ordering::Release);
            }
        },
    );

    assert!(matches!(result, Err(PipelineError::Cancelled)));
    assert!(
        !dest.path().join("120").exists(),
        "no version dir on cancel"
    );
}

#[test]
fn collapses_single_top_level_directory_like_the_ungoogled_zip() {
    // ungoogled-chromium ships `ungoogled-chromium_<tag>_windows_x64/chrome.exe`, so the
    // executable is one level below the archive root. `exe_path_in_zip` stays "engine.exe".
    let zip_bytes = build_zip(&[
        (
            "ungoogled-chromium_151.0.7922.137-1.1_windows_x64/engine.exe",
            b"MZ-fake-exe",
        ),
        (
            "ungoogled-chromium_151.0.7922.137-1.1_windows_x64/locales/en-US.pak",
            b"pak",
        ),
    ]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();

    let installed = install_pipeline(
        &candidate,
        dest.path(),
        &manifest(),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("install");

    // The wrapper directory must be gone: the exe sits at the root of the version dir.
    assert_eq!(
        installed.executable_path,
        dest.path().join("120").join("engine.exe")
    );
    assert!(installed.executable_path.exists());
    assert!(
        dest.path()
            .join("120")
            .join("locales")
            .join("en-US.pak")
            .exists(),
        "sibling files must survive the collapse"
    );
    assert!(
        !dest
            .path()
            .join("120")
            .join("ungoogled-chromium_151.0.7922.137-1.1_windows_x64")
            .exists(),
        "wrapper directory must not be preserved"
    );
}

#[test]
fn does_not_collapse_when_the_root_holds_several_entries() {
    // Engines that zip the exe at the root must keep working unchanged.
    let zip_bytes = build_zip(&[("engine.exe", b"MZ"), ("readme.txt", b"hi")]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();

    let installed = install_pipeline(
        &candidate,
        dest.path(),
        &manifest(),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("install");

    assert_eq!(
        installed.executable_path,
        dest.path().join("120").join("engine.exe")
    );
    assert!(dest.path().join("120").join("readme.txt").exists());
}

#[test]
fn single_top_level_directory_without_the_executable_still_fails() {
    // Collapsing must not turn "wrong archive" into a silent success.
    let zip_bytes = build_zip(&[("some-engine-1.0/readme.txt", b"hello")]);
    let tmp = TempDir::new().unwrap();
    let url = write_zip_to_disk(&tmp, "engine.zip", &zip_bytes);
    let candidate = candidate(&zip_bytes, url);
    let dest = TempDir::new().unwrap();

    let result = install_pipeline(
        &candidate,
        dest.path(),
        &manifest(),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );

    assert!(matches!(result, Err(PipelineError::ExecutableMissing)));
    assert!(!dest.path().join("120").exists());
}
