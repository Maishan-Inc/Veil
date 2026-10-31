use serde::{Deserialize, Serialize};
use veil_core::Fingerprint;

/// A persisted profile row. Timestamps are RFC 3339 strings.
///
/// `PartialEq` without `Eq`: `Fingerprint::device_pixel_ratio` is a float, so the fingerprint is only
/// partially ordered. Nothing keys a map or a set on a profile record, so the bound was never used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRecord {
    pub id: String,
    pub name: String,
    pub avatar_color: String,
    pub group_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    pub browser_version: String,
    pub notes: String,
    pub status: String,
    pub startup_page: String,
    pub startup_url: String,
    pub fingerprint: Fingerprint,
    pub created_at: String,
    pub updated_at: String,
    pub last_launched_at: Option<String>,
}

/// Fields accepted when creating a profile.
#[derive(Debug, Clone)]
pub struct CreateProfile {
    pub name: String,
    pub avatar_color: String,
    pub group_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    pub browser_version: String,
    pub notes: String,
    pub startup_page: String,
    pub startup_url: String,
    pub fingerprint: Fingerprint,
}

/// Fields accepted when updating a profile (id is passed separately).
#[derive(Debug, Clone)]
pub struct UpdateProfile {
    pub name: String,
    pub group_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    pub browser_version: String,
    pub notes: String,
    pub startup_page: String,
    pub startup_url: String,
    pub fingerprint: Fingerprint,
}
