//! End-to-end install pipeline: download → verify → extract → finalize, with progress and
//! cancellation.
//!
//! Staging is done in a tempfile so a cancel/fail leaves the existing engine installation
//! intact. The version directory is created in `dest_root` only on success.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use zip::ZipArchive;

use crate::installer::{download_streaming, InstallError};
use crate::manifest::EngineManifest;
use crate::provider::RuntimeCandidate;
use crate::registry::InstalledRuntime;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("install was cancelled")]
    Cancelled,
    #[error("engine executable missing from archive")]
    ExecutableMissing,
    #[error(transparent)]
    Install(#[from] InstallError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// A snapshot of where the install pipeline is. Tagged-union shape so the IPC layer can
/// re-serialize it directly.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Downloading {
        downloaded: u64,
        total: Option<u64>,
        bytes_per_sec: u64,
    },
    Verifying,
    Extracting {
        entries_done: u32,
        entries_total: u32,
    },
    Finalizing,
    Completed,
}

/// Run the full pipeline for a candidate, writing the engine into
/// `dest_root.join(&candidate.version)` on success.
///
/// `cancel` is checked between every chunk / zip entry / pipeline stage. `on_progress` is
/// called frequently — the application service may throttle if needed before forwarding to
/// the IPC layer.
pub fn install_pipeline<F: FnMut(Progress)>(
    candidate: &RuntimeCandidate,
    dest_root: &Path,
    manifest: &EngineManifest,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> Result<InstalledRuntime, PipelineError> {
    let staging = tempfile::tempdir()?;
    let zip_path = staging.path().join("engine.zip");

    // ---- Download ----
    download_into(
        &candidate.download_url,
        &zip_path,
        cancel.clone(),
        &mut on_progress,
    )?;
    check_cancel(&cancel)?;

    // ---- Verify ----
    on_progress(Progress::Verifying);
    verify_file_sha256(&zip_path, &candidate.sha256)?;
    check_cancel(&cancel)?;

    // ---- Extract into staging ----
    let extracted = staging.path().join("extracted");
    extract_with_progress(&zip_path, &extracted, cancel.clone(), &mut on_progress)?;
    check_cancel(&cancel)?;

    // ---- Finalize ----
    on_progress(Progress::Finalizing);
    // Engines differ in zip layout: some put `chrome.exe` at the archive root, ungoogled-chromium
    // wraps everything in a single `ungoogled-chromium_<tag>_windows_x64/` directory. Collapse a
    // lone top-level directory so `exe_path_in_zip` stays engine-agnostic.
    let extracted = collapse_single_root(&extracted)?;
    let exe_rel = PathBuf::from(&manifest.exe_path_in_zip);
    let exe_in_staging = extracted.join(&exe_rel);
    if !exe_in_staging.exists() {
        return Err(PipelineError::ExecutableMissing);
    }

    let version_dir = dest_root.join(&candidate.version);
    move_into_place(&extracted, &version_dir)?;
    let executable_path = version_dir.join(&exe_rel);

    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default();
    // The only place an engine's identity is knowable: the manifest that chose this download. Recorded
    // on the entry rather than read back from the manifest at launch time, because `engine.toml` is a
    // declaration about the engine Veil installs *today* and the registry may hold engines installed
    // under older ones — which is exactly how the dialect flip (5.5a) turned a conservative mismatch
    // into an over-claiming one.
    let installed = InstalledRuntime {
        version: candidate.version.clone(),
        channel: manifest.channel.clone(),
        executable_path,
        installed_at: now,
        sha256: candidate.sha256.clone(),
        size_bytes: candidate.size_bytes,
        flag_dialect: Some(manifest.flag_dialect),
        engine_name: Some(manifest.name.clone()),
        github_owner: Some(manifest.github_owner.clone()),
        github_repo: Some(manifest.github_repo.clone()),
    };

    on_progress(Progress::Completed);
    Ok(installed)
}

fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<(), PipelineError> {
    if cancel.load(Ordering::Acquire) {
        Err(PipelineError::Cancelled)
    } else {
        Ok(())
    }
}

/// If `root` holds exactly one entry and that entry is a directory, return the directory;
/// otherwise return `root` unchanged.
///
/// This normalizes the two zip layouts engines use — `chrome.exe` at the archive root, or nested
/// under one version-named folder — so a single `exe_path_in_zip` works for both. It deliberately
/// collapses only one level and only when the directory is alone: a zip with several top-level
/// entries is already rooted correctly, and descending further could skip real content.
fn collapse_single_root(root: &Path) -> Result<PathBuf, PipelineError> {
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, io::Error>>()?;
    if entries.len() == 1 {
        let only = entries.remove(0);
        if only.file_type()?.is_dir() {
            return Ok(only.path());
        }
    }
    Ok(root.to_path_buf())
}

fn download_into<F: FnMut(Progress)>(
    url: &str,
    target: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: &mut F,
) -> Result<(), PipelineError> {
    if url.starts_with("file://") || Path::new(url).is_absolute() {
        return copy_local_file(url, target, on_progress);
    }
    let mut file = BufWriter::new(File::create(target)?);
    let start = Instant::now();
    let cancel_for_cb = cancel.clone();
    let mut cancelled = false;
    let result = download_streaming(url, &mut file, |downloaded, total| {
        if cancel_for_cb.load(Ordering::Acquire) {
            // Returning `false` stops the transfer at this chunk. Reporting the flag without it
            // would leave a cancelled install downloading the remaining ~200 MB before anyone
            // noticed — measured, and the reason the callback has a return value at all.
            cancelled = true;
            return false;
        }
        let elapsed = start.elapsed().as_secs_f64().max(0.001);
        let bytes_per_sec = (downloaded as f64 / elapsed) as u64;
        on_progress(Progress::Downloading {
            downloaded,
            total,
            bytes_per_sec,
        });
        true
    });
    file.flush().ok();
    drop(file);

    if cancelled {
        return Err(PipelineError::Cancelled);
    }
    result.map(|_| ()).map_err(PipelineError::Install)
}

fn copy_local_file<F: FnMut(Progress)>(
    url: &str,
    target: &Path,
    on_progress: &mut F,
) -> Result<(), PipelineError> {
    let src_path = if let Some(stripped) = url.strip_prefix("file://") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(url)
    };
    let bytes = fs::read(&src_path)?;
    let total = bytes.len() as u64;
    on_progress(Progress::Downloading {
        downloaded: 0,
        total: Some(total),
        bytes_per_sec: 0,
    });
    fs::write(target, &bytes)?;
    on_progress(Progress::Downloading {
        downloaded: total,
        total: Some(total),
        bytes_per_sec: total,
    });
    Ok(())
}

fn verify_file_sha256(path: &Path, expected: &str) -> Result<(), PipelineError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let actual = hasher.finalize();
    let hex = bytes_to_hex(&actual);
    if hex.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(PipelineError::Install(InstallError::ChecksumMismatch))
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn extract_with_progress<F: FnMut(Progress)>(
    zip_path: &Path,
    destination: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: &mut F,
) -> Result<(), PipelineError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    fs::create_dir_all(destination)?;
    let total = archive.len() as u32;
    on_progress(Progress::Extracting {
        entries_done: 0,
        entries_total: total,
    });

    for index in 0..archive.len() {
        if cancel.load(Ordering::Acquire) {
            // Best-effort cleanup; staging is a tempdir so it's cleaned at end of scope too.
            let _ = fs::remove_dir_all(destination);
            return Err(PipelineError::Cancelled);
        }
        write_entry(&mut archive, index, destination)?;
        on_progress(Progress::Extracting {
            entries_done: (index as u32) + 1,
            entries_total: total,
        });
    }
    Ok(())
}

fn write_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    index: usize,
    destination: &Path,
) -> Result<(), PipelineError> {
    let mut entry = archive.by_index(index)?;
    let relative: PathBuf = match entry.enclosed_name() {
        Some(name) => name.to_path_buf(),
        None => return Err(PipelineError::Install(InstallError::UnsafeZipEntry)),
    };
    let out_path = destination.join(relative);
    if entry.is_dir() {
        fs::create_dir_all(&out_path)?;
        return Ok(());
    }
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = File::create(&out_path)?;
    io::copy(&mut entry, &mut out)?;
    Ok(())
}

/// Move a staging directory into its final spot. Replaces an existing version dir.
fn move_into_place(staging: &Path, target: &Path) -> Result<(), PipelineError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if target.exists() {
        fs::remove_dir_all(target)?;
    }
    // `fs::rename` works across the same filesystem; tempdir + dest_root should be on the same
    // volume in practice. Fallback to copy-then-remove for cross-volume edge cases.
    if fs::rename(staging, target).is_err() {
        copy_dir_all(staging, target)?;
        fs::remove_dir_all(staging).ok();
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
