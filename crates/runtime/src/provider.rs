use serde::{Deserialize, Deserializer};
use std::time::Duration;
use thiserror::Error;

use crate::manifest::{ChecksumSource, EngineManifest};

/// Deserialize a field that GitHub may send as an explicit `null` into `T::default()`.
///
/// `#[serde(default)]` alone only covers a *missing* key; a present `null` still fails for a
/// non-`Option` type. `ungoogled-chromium-windows` publishes releases with `"body": null`, which
/// made the whole response fail to parse.
fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub body: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: Option<u64>,
    /// GitHub-computed content digest, e.g. `sha256:c7b3c5af…`. Present on release assets hosted
    /// by GitHub and used by [`ChecksumSource::AssetDigest`].
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCandidate {
    /// Bare version string (capture group 1 of `version_regex`, or `tag_name` as fallback).
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: Option<u64>,
    pub published_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network unavailable")]
    Network,
    /// GitHub was reachable but the exchange was unusable. The payload distinguishes a bad HTTP
    /// status from an unparseable body — both are one user-facing error, but conflating them in
    /// the type makes this failure undiagnosable from a log.
    #[error("github exchange failed: {0}")]
    Response(&'static str),
    #[error("no compatible asset found")]
    AssetNotFound,
}

/// Select every release that ships an engine asset matching the manifest, newest version first.
/// Drafts/prereleases skipped, license-gated releases skipped.
///
/// The GitHub API's release order is **not** dependable — `ungoogled-chromium-windows` returns
/// releases neither version-ordered nor date-ordered — so candidates are sorted explicitly by
/// numeric version.
pub fn select_candidates(
    releases: &[GitHubRelease],
    manifest: &EngineManifest,
) -> Vec<RuntimeCandidate> {
    select_candidates_with_text_fetcher(releases, manifest, &fetch_text)
}

fn select_candidates_with_text_fetcher(
    releases: &[GitHubRelease],
    manifest: &EngineManifest,
    fetch_text_fn: &dyn Fn(&str) -> Option<String>,
) -> Vec<RuntimeCandidate> {
    let mut candidates: Vec<RuntimeCandidate> = releases
        .iter()
        .filter_map(|release| select_one(release, manifest, fetch_text_fn))
        .collect();
    // Stable sort: releases sharing a version key keep the order GitHub returned them in.
    candidates.sort_by_key(|candidate| std::cmp::Reverse(version_key(&candidate.version)));
    candidates
}

/// Component-wise version key for ordering. Splitting on `.` and `-` and comparing numerically
/// is required because a lexicographic compare ranks `151.0.7922.75` above `151.0.7922.108`.
///
/// Non-numeric components sort as 0, which is why every supported tag scheme must be entirely
/// numeric after the Chromium version. `Veil-Chromium` therefore marks its builds by raising the
/// numeric packaging revision (`151.0.7922.137-1.1000`) rather than with an alphabetic suffix
/// such as `-veil.1`, which would collapse to 0 here and rank releases wrongly. Loosening
/// `version_regex` to accept letters requires changing this function in the same commit.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-'])
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

/// True when a release is a paid / license-gated build.
///
/// Matches a `-pro` tag suffix or a standalone `pro` word in the release name — deliberately not
/// any substring, which would drop legitimate names containing words like "improvements".
fn is_license_gated(release: &GitHubRelease) -> bool {
    let tag = release.tag_name.to_ascii_lowercase();
    if tag.ends_with("-pro") || tag.ends_with(".pro") {
        return true;
    }
    if release
        .name
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "pro")
    {
        return true;
    }
    release.body.to_ascii_lowercase().contains("license key")
}

fn select_one(
    release: &GitHubRelease,
    manifest: &EngineManifest,
    fetch_text_fn: &dyn Fn(&str) -> Option<String>,
) -> Option<RuntimeCandidate> {
    if release.draft || release.prerelease {
        return None;
    }
    if is_license_gated(release) {
        return None;
    }
    let version = extract_version(&release.tag_name, manifest)?;
    let asset = release
        .assets
        .iter()
        .find(|asset| manifest.asset_regex.is_match(&asset.name))?;
    let sha256 = match manifest.checksum_source {
        ChecksumSource::ReleaseBody => extract_sha256(&release.body)?,
        ChecksumSource::AssetDigest => parse_asset_digest(asset.digest.as_deref())?,
        ChecksumSource::SidecarFile => {
            if let Some(hash) = parse_sha256sums(&release.body, &asset.name) {
                hash
            } else {
                // Find the SHA256SUMS sidecar asset published alongside the main asset.
                let sidecar_url = release
                    .assets
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS"))
                    .map(|a| a.browser_download_url.as_str())
                    .or_else(|| {
                        // Fallback: look for "<asset_name>.sha256" sidecar.
                        let sha_name = format!("{}.sha256", asset.name);
                        release
                            .assets
                            .iter()
                            .find(|a| a.name == sha_name)
                            .map(|a| a.browser_download_url.as_str())
                    })?;
                let content = fetch_text_fn(sidecar_url)?;
                parse_sha256sums(&content, &asset.name)?
            }
        }
    };
    Some(RuntimeCandidate {
        version,
        asset_name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
        sha256,
        size_bytes: asset.size,
        published_at: release.published_at.clone(),
    })
}

fn extract_version(tag: &str, manifest: &EngineManifest) -> Option<String> {
    let captures = manifest.version_regex.captures(tag)?;
    let value = captures
        .get(1)
        .map(|m| m.as_str())
        .unwrap_or_else(|| captures.get(0).expect("regex matched").as_str());
    Some(value.to_string())
}

/// Extract the first 64-hex-character SHA-256 token from release notes.
pub fn extract_sha256(body: &str) -> Option<String> {
    body.split(|c: char| !c.is_ascii_hexdigit())
        .find(|part| part.len() == 64 && part.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|part| part.to_ascii_lowercase())
}

/// Parse a GitHub release asset `digest` field (`sha256:<64 hex>`) into a bare lowercase hash.
///
/// Only `sha256:` is accepted: the install pipeline verifies with SHA-256, so silently passing a
/// digest computed with another algorithm through would make verification compare unlike values.
pub fn parse_asset_digest(digest: Option<&str>) -> Option<String> {
    let hash = digest?.trim().strip_prefix("sha256:")?.trim();
    if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hash.to_ascii_lowercase())
    } else {
        None
    }
}

/// Parse a `sha256sum`-style sidecar file and return the hash for `asset_name`.
///
/// Format: each data line is `<64-hex>  <filename>` (one or two spaces). Lines starting
/// with `version=` or `#` are metadata/comments and are skipped.
pub fn parse_sha256sums(content: &str, asset_name: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version=") || line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Split on whitespace: first token = hash, remaining (trimmed) = filename.
        let mut parts = line.splitn(2, |c: char| c.is_ascii_whitespace());
        let hash = parts.next()?.trim();
        let name = parts.next()?.trim();
        if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) && name == asset_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Fetch a small text resource (e.g. a SHA256SUMS sidecar) and return its body.
/// Returns `None` on any network or HTTP error — the caller maps that to "no candidate".
fn fetch_text(url: &str) -> Option<String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("veil-runtime/0")
        .build()
        .ok()?
        .get(url)
        .send()
        .ok()
        .filter(|r| r.status().is_success())
        .and_then(|r| r.text().ok())
}

/// Fetch and parse the GitHub Releases JSON for the engine described by `manifest`.
///
/// Returns `ProviderError::AssetNotFound` when the manifest is still a placeholder so callers
/// can map it to the `runtime.assetNotFound` user-facing code without ever hitting the network.
pub fn fetch_releases(manifest: &EngineManifest) -> Result<Vec<GitHubRelease>, ProviderError> {
    let url = manifest
        .releases_api_url()
        .ok_or(ProviderError::AssetNotFound)?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("veil-runtime/0")
        .build()
        .map_err(|_| ProviderError::Network)?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|_| ProviderError::Network)?;
    if !response.status().is_success() {
        return Err(ProviderError::Response("unexpected http status"));
    }
    let releases: Vec<GitHubRelease> = response
        .json()
        .map_err(|_| ProviderError::Response("release json did not match the expected shape"))?;
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn fixture_manifest() -> EngineManifest {
        // The bundled fixture targets `engine-*-windows-x64*.zip` assets.
        EngineManifest::from_str(
            r#"
            [engine]
            name = "Engine"
            github_owner = "example"
            github_repo = "engine"
            version_regex = "^v?([0-9]+(?:\\.[0-9]+)*)$"
            asset_regex = "^engine-.*windows.*x64.*\\.zip$"
            exe_path_in_zip = "engine.exe"
            checksum_source = "release_body"
            channel = "free"
            "#,
        )
        .expect("manifest")
    }

    /// A sidecar-checksum engine. Third-party engines still publish `SHA256SUMS`, so this path
    /// stays covered even though the bundled engine uses `asset_digest`.
    fn sidecar_manifest() -> EngineManifest {
        EngineManifest::from_str(
            r#"
            [engine]
            name = "SidecarEngine"
            github_owner = "example"
            github_repo = "sidecar-engine"
            version_regex = "^chromium-v([\\d]+(?:\\.[\\d]+)*)$"
            asset_regex = "^sidecar-engine-windows-x64\\.zip$"
            exe_path_in_zip = "chrome.exe"
            checksum_source = "sidecar_file"
            channel = "free"
            "#,
        )
        .expect("manifest")
    }

    /// Mirrors the bundled `engine.toml` (ungoogled-chromium-windows).
    fn ungoogled_manifest() -> EngineManifest {
        EngineManifest::default()
    }

    fn ungoogled_release(tag: &str, digest: Option<&str>) -> GitHubRelease {
        let asset = format!("ungoogled-chromium_{tag}_windows_x64.zip");
        GitHubRelease {
            tag_name: tag.to_string(),
            name: tag.to_string(),
            // ungoogled-chromium-windows leaves the release body empty.
            body: String::new(),
            prerelease: false,
            draft: false,
            published_at: None,
            assets: vec![
                GitHubAsset {
                    name: format!("ungoogled-chromium_{tag}_installer_x64.exe"),
                    browser_download_url: "https://example.invalid/installer.exe".into(),
                    size: None,
                    digest: Some(format!("sha256:{}", "e".repeat(64))),
                },
                GitHubAsset {
                    name: asset.clone(),
                    browser_download_url: format!("https://example.invalid/{asset}"),
                    size: Some(188 * 1024 * 1024),
                    digest: digest.map(|d| d.to_string()),
                },
            ],
        }
    }

    #[test]
    fn skips_pro_and_drafts_and_selects_free_windows_x64() {
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(include_str!("../tests/fixtures/releases.json")).expect("fixture");
        let manifest = fixture_manifest();
        let candidates = select_candidates(&releases, &manifest);
        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.version, "120");
        assert_eq!(candidate.asset_name, "engine-windows-x64.zip");
        assert_eq!(
            candidate.sha256,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn parse_sha256sums_finds_matching_asset() {
        let content = "version=146.0.7680.177.5\n\
            4a12bcde95fa1bb1beef2b41ab5e5c27c36be78e3be3d0dac8c64d705216670e  sidecar-engine-linux-x64.tar.gz\n\
            b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe  sidecar-engine-windows-x64.zip\n";
        assert_eq!(
            parse_sha256sums(content, "sidecar-engine-windows-x64.zip").as_deref(),
            Some("b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe")
        );
        assert_eq!(
            parse_sha256sums(content, "sidecar-engine-linux-x64.tar.gz").as_deref(),
            Some("4a12bcde95fa1bb1beef2b41ab5e5c27c36be78e3be3d0dac8c64d705216670e")
        );
        // Unknown asset → None.
        assert!(parse_sha256sums(content, "unknown.zip").is_none());
        // Empty content → None.
        assert!(parse_sha256sums("", "anything").is_none());
    }

    #[test]
    fn parse_sha256sums_handles_single_space_and_comments() {
        let content = "# generated by sha256sum\n\
            aabbccdd11223344aabbccdd11223344aabbccdd11223344aabbccdd11223344 file.zip\n";
        assert_eq!(
            parse_sha256sums(content, "file.zip").as_deref(),
            Some("aabbccdd11223344aabbccdd11223344aabbccdd11223344aabbccdd11223344")
        );
    }

    fn sidecar_release(body: &str) -> GitHubRelease {
        GitHubRelease {
            tag_name: "chromium-v146.0.7680.177.5".into(),
            name: "SidecarEngine Chromium".into(),
            body: body.to_string(),
            prerelease: false,
            draft: false,
            published_at: Some("2026-06-01T00:00:00Z".into()),
            assets: vec![
                GitHubAsset {
                    name: "sidecar-engine-windows-x64.zip".into(),
                    browser_download_url: "https://example.invalid/sidecar-engine-windows-x64.zip"
                        .into(),
                    size: Some(562),
                    digest: None,
                },
                GitHubAsset {
                    name: "SHA256SUMS".into(),
                    browser_download_url: "https://example.invalid/SHA256SUMS".into(),
                    size: Some(216),
                    digest: None,
                },
            ],
        }
    }

    #[test]
    fn sidecar_source_selects_candidate_without_network() {
        let releases = vec![sidecar_release("")];
        let sidecar = "version=146.0.7680.177.5\n\
            4a12bcde95fa1bb1beef2b41ab5e5c27c36be78e3be3d0dac8c64d705216670e  sidecar-engine-linux-x64.tar.gz\n\
            b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe  sidecar-engine-windows-x64.zip\n";

        let candidates =
            select_candidates_with_text_fetcher(&releases, &sidecar_manifest(), &|url| {
                assert_eq!(url, "https://example.invalid/SHA256SUMS");
                Some(sidecar.to_string())
            });

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].version, "146.0.7680.177.5");
        assert_eq!(candidates[0].asset_name, "sidecar-engine-windows-x64.zip");
        assert_eq!(
            candidates[0].sha256,
            "b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe"
        );
    }

    #[test]
    fn sidecar_source_uses_release_body_checksum_before_fetching_sidecar() {
        let releases = vec![sidecar_release(
            "### Checksums\n\
             b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe  sidecar-engine-windows-x64.zip\n",
        )];

        let candidates =
            select_candidates_with_text_fetcher(&releases, &sidecar_manifest(), &|_| {
                panic!("sidecar should not be fetched")
            });

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].sha256,
            "b213795cb32c3169f766c74ce1d0275fc89d3df256de39c04da7fb4c23b7fdbe"
        );
    }

    #[test]
    fn sidecar_source_returns_empty_when_sidecar_missing_hash() {
        let releases = vec![sidecar_release("")];

        let candidates =
            select_candidates_with_text_fetcher(&releases, &sidecar_manifest(), &|_| {
                Some("version=146.0.7680.177.5\n".to_string())
            });

        assert!(candidates.is_empty());
    }

    #[test]
    fn returns_empty_when_no_checksum_present() {
        let releases = vec![GitHubRelease {
            tag_name: "v1".into(),
            name: "Engine v1".into(),
            body: "no checksum here".into(),
            prerelease: false,
            draft: false,
            published_at: None,
            assets: vec![GitHubAsset {
                name: "engine-windows-x64.zip".into(),
                browser_download_url: "https://example.invalid/x.zip".into(),
                size: None,
                digest: None,
            }],
        }];
        assert!(select_candidates(&releases, &fixture_manifest()).is_empty());
    }

    #[test]
    fn parse_asset_digest_accepts_sha256_and_rejects_everything_else() {
        let hash = "c7b3c5afb01e2dbaaae5500873b8d6ecd9fe4b00ed75cdc2a1ba4e61026336b2";
        assert_eq!(
            parse_asset_digest(Some(&format!("sha256:{hash}"))).as_deref(),
            Some(hash)
        );
        // Uppercase is normalized.
        assert_eq!(
            parse_asset_digest(Some(&format!("sha256:{}", hash.to_ascii_uppercase()))).as_deref(),
            Some(hash)
        );
        assert!(parse_asset_digest(None).is_none());
        // A different algorithm must not be passed through: the pipeline verifies with SHA-256.
        assert!(parse_asset_digest(Some(&format!("sha512:{hash}"))).is_none());
        // Bare hash without the prefix, truncated hash, and non-hex are all rejected.
        assert!(parse_asset_digest(Some(hash)).is_none());
        assert!(parse_asset_digest(Some("sha256:abc")).is_none());
        assert!(parse_asset_digest(Some(&format!("sha256:{}", "z".repeat(64)))).is_none());
    }

    #[test]
    fn asset_digest_source_selects_the_x64_zip_without_network() {
        let hash = "c7b3c5afb01e2dbaaae5500873b8d6ecd9fe4b00ed75cdc2a1ba4e61026336b2";
        let releases = vec![ungoogled_release(
            "151.0.7922.137-1.1",
            Some(&format!("sha256:{hash}")),
        )];

        let candidates =
            select_candidates_with_text_fetcher(&releases, &ungoogled_manifest(), &|_| {
                panic!("asset_digest must not fetch anything")
            });

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].version, "151.0.7922.137-1.1");
        assert_eq!(
            candidates[0].asset_name,
            "ungoogled-chromium_151.0.7922.137-1.1_windows_x64.zip"
        );
        assert_eq!(candidates[0].sha256, hash);
    }

    #[test]
    fn asset_digest_source_skips_releases_without_a_digest() {
        let releases = vec![ungoogled_release("151.0.7922.137-1.1", None)];
        assert!(select_candidates(&releases, &ungoogled_manifest()).is_empty());
    }

    #[test]
    fn candidates_are_sorted_by_numeric_version_not_api_order() {
        // The order GitHub actually returns for ungoogled-chromium-windows: neither
        // version-ordered nor date-ordered, and `.75` arrives before `.108`.
        let digest = format!("sha256:{}", "a".repeat(64));
        let releases: Vec<GitHubRelease> = [
            "151.0.7922.137-1.1",
            "151.0.7922.75-1.1",
            "151.0.7922.108-1.1",
            "150.0.7871.186-1.1",
        ]
        .iter()
        .map(|tag| ungoogled_release(tag, Some(&digest)))
        .collect();

        let versions: Vec<String> = select_candidates(&releases, &ungoogled_manifest())
            .into_iter()
            .map(|c| c.version)
            .collect();

        assert_eq!(
            versions,
            vec![
                "151.0.7922.137-1.1",
                "151.0.7922.108-1.1",
                "151.0.7922.75-1.1",
                "150.0.7871.186-1.1",
            ]
        );
    }

    #[test]
    fn packaging_revision_breaks_ties_within_one_chromium_version() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let releases: Vec<GitHubRelease> = ["151.0.7922.137-1.1", "151.0.7922.137-1.2"]
            .iter()
            .map(|tag| ungoogled_release(tag, Some(&digest)))
            .collect();

        let versions: Vec<String> = select_candidates(&releases, &ungoogled_manifest())
            .into_iter()
            .map(|c| c.version)
            .collect();

        assert_eq!(versions, vec!["151.0.7922.137-1.2", "151.0.7922.137-1.1"]);
    }

    #[test]
    fn veil_fork_tag_sorts_above_the_upstream_tag_it_forked_from() {
        // `Veil-Chromium` tags its builds by raising the *packaging* revision to `1000`
        // (`revision.txt` in the fork root, which `package.py::_get_packaging_revision` reads),
        // so the marker is numeric and `version_key` orders it correctly. An alphabetic marker
        // such as `-veil.1` would both fail `version_regex` and sort as 0 here.
        //
        // `engine.toml` points at exactly one repository, so upstream and fork tags never share
        // a list in production — but they do share a machine's install directory, and a fork
        // build must never be ranked below the upstream release it was cut from.
        let digest = format!("sha256:{}", "a".repeat(64));
        let releases: Vec<GitHubRelease> = [
            "151.0.7922.137-1.1",    // upstream, the tag the fork was cut from
            "151.0.7922.137-1.2",    // a hypothetical upstream rebuild of the same version
            "151.0.7922.137-1.1000", // Veil-Chromium
        ]
        .iter()
        .map(|tag| ungoogled_release(tag, Some(&digest)))
        .collect();

        let versions: Vec<String> = select_candidates(&releases, &ungoogled_manifest())
            .into_iter()
            .map(|c| c.version)
            .collect();

        assert_eq!(
            versions,
            vec![
                "151.0.7922.137-1.1000",
                "151.0.7922.137-1.2",
                "151.0.7922.137-1.1",
            ]
        );
        // A newer Chromium version still wins over the fork marker: the marker distinguishes
        // builds of one version, it does not pin the fork at the top forever.
        assert!(version_key("151.0.7922.150-1.1") > version_key("151.0.7922.137-1.1000"));
    }

    #[test]
    fn license_gating_matches_pro_tags_but_not_substrings() {
        let digest = format!("sha256:{}", "a".repeat(64));
        // A `-pro` tag suffix is gated (this is what killed the previous engine target).
        let mut pro = ungoogled_release("151.0.7922.137-1.1", Some(&digest));
        pro.tag_name = "chromium-v150.0.7871.114.6-pro".into();
        assert!(is_license_gated(&pro));

        // A standalone "Pro" word in the name is gated.
        let mut named = ungoogled_release("151.0.7922.137-1.1", Some(&digest));
        named.name = "Engine 151 Pro".into();
        assert!(is_license_gated(&named));

        // "pro" as a substring of an ordinary word is NOT gated.
        let mut ordinary = ungoogled_release("151.0.7922.137-1.1", Some(&digest));
        ordinary.name = "151 with protocol improvements".into();
        assert!(!is_license_gated(&ordinary));
        assert_eq!(
            select_candidates(&[ordinary], &ungoogled_manifest()).len(),
            1
        );
    }

    #[test]
    fn parses_releases_with_null_name_and_body() {
        // Real shape returned by ungoogled-chromium-windows: `body` is JSON null, not "".
        // `#[serde(default)]` alone does not cover an explicit null, which made the entire
        // response fail to deserialize and surfaced as "unexpected github status".
        let json = r#"[{
            "tag_name": "151.0.7922.137-1.1",
            "name": null,
            "body": null,
            "prerelease": false,
            "draft": false,
            "published_at": "2026-08-16T02:23:53Z",
            "assets": [{
                "name": "ungoogled-chromium_151.0.7922.137-1.1_windows_x64.zip",
                "browser_download_url": "https://example.invalid/x.zip",
                "size": 197089929,
                "digest": "sha256:c7b3c5afb01e2dbaaae5500873b8d6ecd9fe4b00ed75cdc2a1ba4e61026336b2"
            }]
        }]"#;

        let releases: Vec<GitHubRelease> = serde_json::from_str(json).expect("null-tolerant parse");
        assert_eq!(releases[0].name, "");
        assert_eq!(releases[0].body, "");

        let candidates = select_candidates(&releases, &ungoogled_manifest());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].version, "151.0.7922.137-1.1");
        assert_eq!(
            candidates[0].sha256,
            "c7b3c5afb01e2dbaaae5500873b8d6ecd9fe4b00ed75cdc2a1ba4e61026336b2"
        );
        assert_eq!(candidates[0].size_bytes, Some(197089929));
    }

    #[test]
    fn fetch_releases_returns_asset_not_found_for_placeholder_manifest() {
        // Placeholder manifests must never touch the network, even when the bundled manifest is real.
        let manifest = EngineManifest::from_str(
            r#"
            [engine]
            name = "Placeholder"
            github_owner = "TBD"
            github_repo = "TBD"
            version_regex = "^v?([0-9]+(?:\\.[0-9]+)*)$"
            asset_regex = "^engine-windows-x64\\.zip$"
            exe_path_in_zip = "engine.exe"
            checksum_source = "release_body"
            channel = "free"
            "#,
        )
        .expect("manifest");
        assert!(matches!(
            fetch_releases(&manifest).unwrap_err(),
            ProviderError::AssetNotFound
        ));
    }
}
