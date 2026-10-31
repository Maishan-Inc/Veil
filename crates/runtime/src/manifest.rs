//! Engine manifest — the *only* place that knows what concrete browser engine Veil targets.
//!
//! The manifest is loaded from `engine.toml` at the crate root (bundled into the binary via
//! `include_str!`). Tests and tools can construct a manifest in memory through
//! [`EngineManifest::from_str`] or [`EngineManifest::default`].
//!
//! The bundled manifest targets `Maishan-Inc/Veil-Chromium` — Veil's own build of
//! ungoogled-chromium — with `flag_dialect = "veil-patched"` since 2026-08-23, when release
//! `151.0.7922.137-1.1001` became the first build carrying the fingerprint patch set. It shipped
//! `stock-chromium` until then, deliberately: a dialect must never claim a capability the engine
//! lacks, because Chromium ignores unknown switches silently and the profile would be reported as
//! protected. A
//! manifest may still carry `TBD` owner/repo placeholders, which the application layer treats as
//! "no compatible engine selected" and which keeps Veil fully offline.

use std::str::FromStr;

use regex::Regex;
use serde::Deserialize;
use thiserror::Error;

/// Bundled manifest (resolved at compile time). The application layer can override this with
/// [`EngineManifest::from_str`] when a user-supplied manifest needs to win.
pub const BUNDLED_MANIFEST: &str = include_str!("../engine.toml");

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("invalid TOML")]
    Toml(#[from] toml::de::Error),
    #[error("invalid regex `{0}`")]
    InvalidRegex(String),
}

/// How to source an asset's SHA-256 checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumSource {
    /// Pull the first 64-hex-character SHA-256 token from the release notes body.
    #[default]
    ReleaseBody,
    /// Use a `SHA256SUMS` / `<asset>.sha256` sidecar asset published alongside the main asset.
    SidecarFile,
    /// Use the GitHub Releases API's per-asset `digest` field (`sha256:<hex>`). Preferred when
    /// the publisher ships no checksum file: the digest is computed by GitHub, not the publisher,
    /// and it needs no extra network request.
    AssetDigest,
}

/// Which launch-switch vocabulary the installed engine implements.
///
/// Defined in `veil_core` because `veil_core::to_engine_args` takes it as a parameter and
/// `veil_core` depends on no internal crate; re-exported here because `engine.toml`'s
/// `[capabilities] flag_dialect` is where the value comes from.
pub use veil_core::FlagDialect;

#[derive(Debug, Deserialize)]
struct RawDocument {
    engine: RawEngine,
    #[serde(default)]
    capabilities: RawCapabilities,
}

#[derive(Debug, Default, Deserialize)]
struct RawCapabilities {
    #[serde(default)]
    flag_dialect: FlagDialect,
}

#[derive(Debug, Deserialize)]
struct RawEngine {
    name: String,
    github_owner: String,
    github_repo: String,
    version_regex: String,
    asset_regex: String,
    exe_path_in_zip: String,
    #[serde(default)]
    checksum_source: ChecksumSource,
    #[serde(default = "default_channel")]
    channel: String,
}

fn default_channel() -> String {
    "free".to_string()
}

/// Pre-compiled engine targeting metadata.
#[derive(Debug, Clone)]
pub struct EngineManifest {
    pub name: String,
    pub github_owner: String,
    pub github_repo: String,
    pub version_regex: Regex,
    pub asset_regex: Regex,
    pub exe_path_in_zip: String,
    pub checksum_source: ChecksumSource,
    pub channel: String,
    /// Switch vocabulary the engine implements. Defaults to [`FlagDialect::StockChromium`] when
    /// the manifest has no `[capabilities]` section.
    pub flag_dialect: FlagDialect,
}

impl EngineManifest {
    /// True when the manifest still points at the `TBD` placeholder repository.
    pub fn is_placeholder(&self) -> bool {
        self.github_owner.eq_ignore_ascii_case("tbd")
            || self.github_repo.eq_ignore_ascii_case("tbd")
    }

    /// `https://api.github.com/repos/{owner}/{repo}/releases?per_page=100`
    /// (or `None` while placeholder).
    pub fn releases_api_url(&self) -> Option<String> {
        if self.is_placeholder() {
            None
        } else {
            Some(format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=100",
                self.github_owner, self.github_repo
            ))
        }
    }
}

impl Default for EngineManifest {
    /// The bundled `engine.toml`. Panics only if the bundled file is itself malformed, which
    /// is caught by the manifest tests at build time.
    fn default() -> Self {
        EngineManifest::from_str(BUNDLED_MANIFEST).expect("bundled engine.toml is valid")
    }
}

impl FromStr for EngineManifest {
    type Err = ManifestError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let RawDocument {
            engine,
            capabilities,
        } = toml::from_str(input)?;
        let version_regex = compile_ci(&engine.version_regex)?;
        let asset_regex = compile_ci(&engine.asset_regex)?;
        Ok(EngineManifest {
            name: engine.name,
            github_owner: engine.github_owner,
            github_repo: engine.github_repo,
            version_regex,
            asset_regex,
            exe_path_in_zip: engine.exe_path_in_zip,
            checksum_source: engine.checksum_source,
            channel: engine.channel,
            flag_dialect: capabilities.flag_dialect,
        })
    }
}

fn compile_ci(pattern: &str) -> Result<Regex, ManifestError> {
    Regex::new(&format!("(?i){pattern}"))
        .map_err(|_| ManifestError::InvalidRegex(pattern.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifest_parses() {
        let manifest = EngineManifest::default();
        assert_eq!(manifest.name, "Veil-Chromium");
        assert_eq!(manifest.channel, "stable");
        assert_eq!(manifest.github_owner, "Maishan-Inc");
        assert_eq!(manifest.github_repo, "Veil-Chromium");
        assert_eq!(manifest.checksum_source, ChecksumSource::AssetDigest);
        assert_eq!(manifest.exe_path_in_zip, "chrome.exe");
    }

    /// The fork ships `Veil-Chromium`'s own dialect claim. It had to stay `stock-chromium` until the
    /// Tier 1 patch set had landed *and* `to_engine_args` had been corrected, because declaring
    /// `veil-patched` against a patch-free
    /// build re-creates the Stage 1 bug: switches emitted, silently ignored, profile reported as
    /// protected.
    ///
    /// Both conditions were met on 2026-08-23 and the claim is now `veil-patched`. What still guards
    /// the invariant is not this constant but the pair of tests below it and
    /// `fingerprint::tests::veil_patched_never_emits_a_switch_no_tier_one_patch_reads`: the manifest
    /// may only claim a dialect whose every switch has a patch behind it.
    #[test]
    fn bundled_manifest_claims_the_dialect_the_published_engine_implements() {
        assert_eq!(
            EngineManifest::default().flag_dialect,
            FlagDialect::VeilPatched
        );
    }

    #[test]
    fn bundled_manifest_matches_real_ungoogled_tags_and_assets() {
        let manifest = EngineManifest::default();
        // Real tag / asset names. The first pair is the fork's published release; the second is
        // the upstream tag it was cut from, which must keep matching because
        // `ungoogled-software/ungoogled-chromium-windows` is the documented rollback target.
        // `-1.1001` is the patched release the manifest now targets, `-1.1000` the zero-patch build
        // every Stage 5 measurement is diffed against, and `-1.1` the upstream tag both were cut
        // from — the documented rollback target, which must keep matching.
        for tag in [
            "151.0.7922.137-1.1001",
            "151.0.7922.137-1.1000",
            "151.0.7922.137-1.1",
        ] {
            assert_eq!(
                manifest
                    .version_regex
                    .captures(tag)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str()),
                Some(tag)
            );
            assert!(manifest
                .asset_regex
                .is_match(&format!("ungoogled-chromium_{tag}_windows_x64.zip")));
        }
        // Sibling assets from the same release must not match.
        for other in [
            "ungoogled-chromium_151.0.7922.137-1.1_windows_x86.zip",
            "ungoogled-chromium_151.0.7922.137-1.1_windows_arm64.zip",
            "ungoogled-chromium_151.0.7922.137-1.1_installer_x64.exe",
            "ungoogled-chromium_151.0.7922.137-1.1000_installer_x64.exe",
        ] {
            assert!(!manifest.asset_regex.is_match(other), "matched {other}");
        }
    }

    #[test]
    fn version_regex_accepts_the_veil_fork_tag_scheme() {
        // `Veil-Chromium` marks its builds in the *packaging* revision — the position owned by
        // the fork's own root `revision.txt`, which `package.py` reads via
        // `_get_packaging_revision()`. `1000` cannot collide with upstream, whose packaging
        // revision is a small integer that counts rebuilds of one Chromium version.
        let manifest = EngineManifest::default();
        assert_eq!(
            manifest
                .version_regex
                .captures("151.0.7922.137-1.1000")
                .and_then(|c| c.get(1))
                .map(|m| m.as_str()),
            Some("151.0.7922.137-1.1000")
        );
        // The asset name `package.py` derives from the same two revision files must still be
        // pinned by `asset_regex`, and its siblings must still be rejected.
        assert!(manifest
            .asset_regex
            .is_match("ungoogled-chromium_151.0.7922.137-1.1000_windows_x64.zip"));
        assert!(!manifest
            .asset_regex
            .is_match("ungoogled-chromium_151.0.7922.137-1.1000_installer_x64.exe"));
    }

    #[test]
    fn version_regex_rejects_alphabetic_fork_tag_schemes() {
        // A tag like `151.0.7922.137-veil.1` must not be accepted. After the `-`
        // this regex requires digits, so such a tag produces *no candidate at all* — the
        // Browser page shows an empty list, which is indistinguishable from a network failure.
        //
        // Loosening the regex is not the fix: `version_key` parses components with
        // `unwrap_or(0)`, so every alphabetic component would sort as 0 and rank releases
        // wrongly. This test exists so a future prettier-tag change has to confront that.
        let manifest = EngineManifest::default();
        for rejected in [
            "151.0.7922.137-veil.1",
            "151.0.7922.137-veil1",
            "151.0.7922.137-1.1-veil.1",
            "veil-151.0.7922.137-1.1",
        ] {
            assert!(
                manifest.version_regex.captures(rejected).is_none(),
                "version_regex unexpectedly accepted {rejected}"
            );
        }
    }

    #[test]
    fn bundled_manifest_declares_the_patched_dialect() {
        // Release 151.0.7922.137-1.1001 is the first build that carries the patch set, so the
        // manifest may finally claim it. The rule the earlier version of this test enforced still
        // holds in the other direction: a dialect must not be claimed before an engine implements it.
        assert_eq!(
            EngineManifest::default().flag_dialect,
            FlagDialect::VeilPatched
        );
    }

    #[test]
    fn missing_capabilities_section_defaults_to_stock_dialect() {
        let toml = r#"
            [engine]
            name = "X"
            github_owner = "a"
            github_repo = "b"
            version_regex = "^v([0-9]+)$"
            asset_regex = ".*"
            exe_path_in_zip = "x"
        "#;
        let manifest = EngineManifest::from_str(toml).expect("parse");
        assert_eq!(manifest.flag_dialect, FlagDialect::StockChromium);
    }

    #[test]
    fn veil_patched_dialect_parses() {
        let toml = r#"
            [engine]
            name = "Veil-Chromium"
            github_owner = "veil"
            github_repo = "Veil-Chromium"
            version_regex = "^([0-9.]+)$"
            asset_regex = ".*"
            exe_path_in_zip = "chrome.exe"

            [capabilities]
            flag_dialect = "veil-patched"
        "#;
        let manifest = EngineManifest::from_str(toml).expect("parse");
        assert_eq!(manifest.flag_dialect, FlagDialect::VeilPatched);
    }

    #[test]
    fn bundled_manifest_is_not_placeholder() {
        let manifest = EngineManifest::default();
        assert!(!manifest.is_placeholder());
        assert!(manifest.releases_api_url().is_some());
    }

    #[test]
    fn custom_manifest_is_not_placeholder() {
        let toml = r#"
            [engine]
            name = "ExampleEngine"
            github_owner = "example"
            github_repo = "engine"
            version_regex = "^v(\\d+)$"
            asset_regex = "^engine-windows-x64\\.zip$"
            exe_path_in_zip = "bin/engine.exe"
            checksum_source = "release_body"
            channel = "free"
        "#;
        let manifest = EngineManifest::from_str(toml).expect("parse");
        assert!(!manifest.is_placeholder());
        assert_eq!(
            manifest.releases_api_url().as_deref(),
            Some("https://api.github.com/repos/example/engine/releases?per_page=100")
        );
    }

    #[test]
    fn rejects_invalid_regex() {
        let toml = r#"
            [engine]
            name = "X"
            github_owner = "a"
            github_repo = "b"
            version_regex = "("
            asset_regex = ".*"
            exe_path_in_zip = "x"
        "#;
        let err = EngineManifest::from_str(toml).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidRegex(_)));
    }

    #[test]
    fn regexes_are_case_insensitive() {
        let toml = r#"
            [engine]
            name = "X"
            github_owner = "a"
            github_repo = "b"
            version_regex = "^V([0-9]+)$"
            asset_regex = "WINDOWS-X64\\.ZIP$"
            exe_path_in_zip = "x"
        "#;
        let manifest = EngineManifest::from_str(toml).expect("parse");
        assert!(manifest.version_regex.is_match("v120"));
        assert!(manifest.asset_regex.is_match("engine-windows-x64.zip"));
    }
}
