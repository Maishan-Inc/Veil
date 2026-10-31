//! Engine runtime management: discover (GitHub Releases), verify (SHA-256), extract, and
//! register the browser engine. Also home to the **proxy connectivity test** because
//! `crates/runtime` is the only crate allowed to touch the network.
//!
//! Network access is only performed when a service explicitly calls into this crate after
//! a user action (install runtime, manually check for updates, test a proxy). The engine
//! itself is pluggable; the concrete target lives in `engine.toml` and is loaded through
//! [`manifest::EngineManifest`].

pub mod fingerprint_preset;
pub mod installer;
pub mod manifest;
pub mod pipeline;
pub mod provider;
pub mod proxy;
pub mod registry;

pub use fingerprint_preset::{randomize, randomize_for_proxy};
pub use installer::{
    download, download_streaming, extract_zip_safely, sha256_hex, verify_sha256, InstallError,
};
pub use manifest::{ChecksumSource, EngineManifest, FlagDialect, ManifestError};
pub use pipeline::{install_pipeline, PipelineError, Progress};
pub use provider::{
    extract_sha256, fetch_releases, parse_asset_digest, select_candidates, GitHubAsset,
    GitHubRelease, ProviderError, RuntimeCandidate,
};
pub use proxy::{test_proxy, ProxyConfig, ProxyTestError, ProxyTestOutcome};
pub use registry::{InstalledRuntime, RegistryError, RuntimeRegistryFile};
