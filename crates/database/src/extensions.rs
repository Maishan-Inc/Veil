use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserExtensionRecord {
    pub id: String,
    pub name: String,
    pub source: String,
    pub store_url: Option<String>,
    pub store_id: Option<String>,
    pub version: String,
    pub install_path: String,
    pub default_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateBrowserExtension {
    pub name: String,
    pub source: String,
    pub store_url: Option<String>,
    pub store_id: Option<String>,
    pub version: String,
    pub install_path: String,
    pub default_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateBrowserExtension {
    pub name: String,
    pub default_enabled: bool,
}
