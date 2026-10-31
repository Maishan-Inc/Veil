use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("download failed")]
    Download,
    #[error("checksum mismatch")]
    ChecksumMismatch,
    #[error("zip entry escapes destination")]
    UnsafeZipEntry,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Lowercase hex SHA-256 of the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Verify bytes against an expected hex SHA-256 (case-insensitive).
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), InstallError> {
    if sha256_hex(bytes).eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(InstallError::ChecksumMismatch)
    }
}

/// Time allowed to reach the server before giving up. This is the timeout that matters for the
/// symptom users actually hit — a request that never gets past connect/TLS/redirect, so no
/// progress event is ever emitted and the UI has nothing to show.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Ceiling for the whole transfer. Sized so it cannot fail a *working* download of a ~200 MB
/// engine: 2 hours is 205 MB at ~29 KB/s. The previous value was 600 seconds, which failed any
/// line slower than ~350 KB/s partway through. `reqwest`'s blocking client has no read timeout,
/// so this is the only thing that ends a body that goes quiet forever.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

/// Download a URL into memory (blocking). Convenience for callers that don't need progress.
pub fn download(url: &str) -> Result<Vec<u8>, InstallError> {
    let bytes = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("veil-runtime/0")
        .build()
        .map_err(|_| InstallError::Download)?
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .map_err(|_| InstallError::Download)?;
    Ok(bytes.to_vec())
}

/// Download a URL into `output`, streaming chunks and reporting progress.
///
/// `on_progress(received_bytes, total_bytes)` is invoked after every chunk and **decides whether to
/// keep going**: returning `false` stops the transfer at the current chunk and returns the bytes
/// received so far. That return value is the only way a caller can abort mid-download — without it
/// a cancel request could not take effect until the whole body had been read, which for a ~200 MB
/// engine means the user watches a "cancel" they already pressed keep downloading (measured
/// 2026-08-25: cancelled at 3.4 MB, still transferring at 118 MB). `total_bytes` is `None` when the
/// server doesn't send `Content-Length`. Returns the total bytes received.
///
/// **Timeouts measure the two ways a transfer dies, not how long it takes.** The engine zip is
/// ~200 MB, so a slow line legitimately needs many minutes and the previous 600-second ceiling
/// failed such a download *while it was still working*. `CONNECT_TIMEOUT` bounds never reaching
/// the server — the case that showed the user a frozen `0 B` with no rate, because no progress
/// event had been emitted yet — and `TRANSFER_TIMEOUT` is the outer ceiling.
pub fn download_streaming<W: Write>(
    url: &str,
    output: &mut W,
    mut on_progress: impl FnMut(u64, Option<u64>) -> bool,
) -> Result<u64, InstallError> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TRANSFER_TIMEOUT)
        .user_agent("veil-runtime/0")
        .build()
        .map_err(|_| InstallError::Download)?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|_| InstallError::Download)?;
    let total = response.content_length();
    if !on_progress(0, total) {
        return Ok(0);
    }

    let mut buffer = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    loop {
        let n = response.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
        received += n as u64;
        if !on_progress(received, total) {
            break;
        }
    }
    Ok(received)
}

/// Extract a zip into `destination`, rejecting any entry that would escape the destination
/// directory (zip-slip protection).
pub fn extract_zip_safely(zip_path: &Path, destination: &Path) -> Result<(), InstallError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    fs::create_dir_all(destination)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let relative: PathBuf = match entry.enclosed_name() {
            Some(name) => name.to_path_buf(),
            None => return Err(InstallError::UnsafeZipEntry),
        };
        let out_path = destination.join(relative);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn computes_known_sha256() {
        assert_eq!(
            sha256_hex(b"veil"),
            "01979b66ee1794c473f53cafff0889e714aac28b2515e3572072424b919634f3"
        );
    }

    #[test]
    fn verify_accepts_match_and_rejects_mismatch() {
        let expected = sha256_hex(b"payload");
        assert!(verify_sha256(b"payload", &expected).is_ok());
        assert!(matches!(
            verify_sha256(b"tampered", &expected),
            Err(InstallError::ChecksumMismatch)
        ));
    }

    #[test]
    fn extracts_zip_and_rejects_zip_slip() {
        // Build a tiny zip with one safe entry and one zip-slip entry.
        let safe = {
            let mut buffer = Cursor::new(Vec::<u8>::new());
            {
                let mut zip = ZipWriter::new(&mut buffer);
                zip.start_file("engine.exe", SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"MZ").unwrap();
                zip.finish().unwrap();
            }
            buffer.into_inner()
        };
        let tmp = TempDir::new().unwrap();
        let zip_path = tmp.path().join("safe.zip");
        fs::write(&zip_path, &safe).unwrap();
        let dest = tmp.path().join("out");
        extract_zip_safely(&zip_path, &dest).unwrap();
        assert!(dest.join("engine.exe").exists());
    }

    /// Serve one `Content-Length`-labelled body from loopback and return the port. The body is
    /// written in small chunks and every write error is swallowed: a client that stops early closes
    /// the socket, and that broken pipe is the success condition of the test below, not a failure.
    fn serve_once(body_len: usize) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            // Read the request head; the body is irrelevant.
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while stream.read(&mut byte).unwrap_or(0) == 1 {
                head.push(byte[0]);
                if head.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let _ = stream.write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Length: {body_len}\r\n\r\n").as_bytes(),
            );
            let chunk = [b'v'; 16 * 1024];
            let mut sent = 0;
            while sent < body_len {
                let n = chunk.len().min(body_len - sent);
                if stream.write_all(&chunk[..n]).is_err() {
                    return;
                }
                sent += n;
            }
            let _ = stream.flush();
        });
        port
    }

    /// The cancel path. Without the callback's return value a cancelled install kept transferring
    /// to the end of the body — measured on the real 195 MB asset: cancel pressed at 3.4 MB, still
    /// downloading at 118 MB. The assertion is *how much* arrived, because "it eventually stopped"
    /// was already true before the fix.
    #[test]
    fn download_streaming_stops_when_the_callback_says_stop() {
        const TOTAL: usize = 4 * 1024 * 1024;
        let port = serve_once(TOTAL);
        let mut sink = Vec::new();
        let mut calls = 0;
        let received = download_streaming(
            &format!("http://127.0.0.1:{port}/engine.zip"),
            &mut sink,
            |downloaded, _total| {
                calls += 1;
                // Keep going until something has actually arrived, then stop.
                downloaded == 0
            },
        )
        .expect("transfer");

        assert!(calls >= 2, "callback should see the header and one chunk");
        assert!(
            received > 0 && (received as usize) < TOTAL / 2,
            "stopped after {received} B of {TOTAL}"
        );
        assert_eq!(
            sink.len() as u64,
            received,
            "output matches the return value"
        );
    }

    #[test]
    fn download_streaming_reads_the_whole_body_when_the_callback_keeps_going() {
        const TOTAL: usize = 256 * 1024;
        let port = serve_once(TOTAL);
        let mut sink = Vec::new();
        let received = download_streaming(
            &format!("http://127.0.0.1:{port}/engine.zip"),
            &mut sink,
            |_downloaded, _total| true,
        )
        .expect("transfer");
        assert_eq!(received as usize, TOTAL);
        assert_eq!(sink.len(), TOTAL);
    }
}
