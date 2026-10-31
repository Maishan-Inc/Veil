use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode};
use veil_database::{
    BrowserExtensionRecord, CreateBrowserExtension, Database, DatabaseError, UpdateBrowserExtension,
};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExtension {
    pub id: String,
    pub name: String,
    pub source: String,
    pub store_url: Option<String>,
    pub store_id: Option<String>,
    pub version: String,
    pub install_path: String,
    pub default_enabled: bool,
    pub profile_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExtensionFromStoreInput {
    pub store_url: String,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExtensionFromLocalInput {
    pub local_path: String,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExtensionInput {
    pub name: String,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub profile_ids: Vec<String>,
}

#[derive(Clone)]
pub struct ExtensionService {
    db: Arc<Mutex<Database>>,
    install_root: PathBuf,
}

impl ExtensionService {
    pub fn new(db: Arc<Mutex<Database>>, install_root: PathBuf) -> Self {
        Self { db, install_root }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Database> {
        self.db.lock().expect("database lock poisoned")
    }

    pub fn list(&self) -> AppResult<Vec<BrowserExtension>> {
        let db = self.db();
        let records = db.list_browser_extensions().map_err(map_read_error)?;
        records
            .into_iter()
            .map(|record| map_browser_extension(&db, record))
            .collect()
    }

    pub fn get(&self, id: &str) -> AppResult<BrowserExtension> {
        let db = self.db();
        let record = db.get_browser_extension(id).map_err(map_read_error)?;
        map_browser_extension(&db, record)
    }

    pub fn import_from_store(
        &self,
        input: ImportExtensionFromStoreInput,
    ) -> AppResult<BrowserExtension> {
        let store_url = input.store_url.trim();
        let store_id = parse_chrome_store_id(store_url).ok_or(ErrorCode::ExtensionInvalid)?;
        let bytes =
            download_chrome_extension(&store_id).map_err(|_| ErrorCode::ExtensionDownloadFailed)?;
        let install_id = uuid::Uuid::new_v4().to_string();
        let install_dir = self.install_root.join(&install_id);
        extract_crx_payload(&bytes, &install_dir).map_err(|_| ErrorCode::ExtensionInvalid)?;
        let manifest = read_extension_manifest(&install_dir)?;
        let name = manifest.name.unwrap_or_else(|| store_id.clone());
        let version = manifest.version.unwrap_or_default();

        let db = self.db();
        let record = db
            .create_browser_extension(CreateBrowserExtension {
                name,
                source: "chromeStore".to_string(),
                store_url: Some(store_url.to_string()),
                store_id: Some(store_id),
                version,
                install_path: install_dir.to_string_lossy().to_string(),
                default_enabled: input.default_enabled,
            })
            .map_err(map_write_error)?;
        db.replace_extension_profiles(&record.id, &dedupe_ids(input.profile_ids))
            .map_err(map_write_error)?;
        map_browser_extension(&db, record)
    }

    pub fn import_from_local(
        &self,
        input: ImportExtensionFromLocalInput,
    ) -> AppResult<BrowserExtension> {
        let source = PathBuf::from(input.local_path.trim());
        if source.as_os_str().is_empty() || !source.exists() {
            return Err(ErrorCode::ExtensionInvalid.into());
        }
        let source = source
            .canonicalize()
            .map_err(|_| AppError::from(ErrorCode::ExtensionInvalid))?;
        let install_id = uuid::Uuid::new_v4().to_string();
        let install_dir = self.install_root.join(&install_id);
        if source.is_dir() {
            copy_dir_safely(&source, &install_dir).map_err(|_| ErrorCode::ExtensionInvalid)?;
        } else if source
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("crx"))
        {
            let bytes = fs::read(&source).map_err(|_| ErrorCode::ExtensionInvalid)?;
            extract_crx_payload(&bytes, &install_dir).map_err(|_| ErrorCode::ExtensionInvalid)?;
        } else {
            extract_zip_safely(&source, &install_dir).map_err(|_| ErrorCode::ExtensionInvalid)?;
        }
        let manifest = read_extension_manifest(&install_dir)?;
        let fallback_name = source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Extension")
            .to_string();
        let name = manifest.name.unwrap_or(fallback_name);
        let version = manifest.version.unwrap_or_default();

        let db = self.db();
        let record = db
            .create_browser_extension(CreateBrowserExtension {
                name,
                source: "local".to_string(),
                store_url: None,
                store_id: None,
                version,
                install_path: install_dir.to_string_lossy().to_string(),
                default_enabled: input.default_enabled,
            })
            .map_err(map_write_error)?;
        db.replace_extension_profiles(&record.id, &dedupe_ids(input.profile_ids))
            .map_err(map_write_error)?;
        map_browser_extension(&db, record)
    }

    pub fn update(&self, id: &str, input: UpdateExtensionInput) -> AppResult<BrowserExtension> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(ErrorCode::ExtensionInvalid.into());
        }
        let db = self.db();
        let record = db
            .update_browser_extension(
                id,
                UpdateBrowserExtension {
                    name: name.to_string(),
                    default_enabled: input.default_enabled,
                },
            )
            .map_err(map_write_error)?;
        db.replace_extension_profiles(id, &dedupe_ids(input.profile_ids))
            .map_err(map_write_error)?;
        map_browser_extension(&db, record)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let install_path = self
            .db()
            .get_browser_extension(id)
            .map_err(map_read_error)?
            .install_path;
        self.db()
            .delete_browser_extension(id)
            .map_err(map_write_error)?;
        let _ = remove_dir_if_under(&self.install_root, Path::new(&install_path));
        Ok(())
    }

    pub fn installed_for_profile(&self, profile_id: &str) -> AppResult<Vec<PathBuf>> {
        let db = self.db();
        let extensions = db
            .list_profile_extensions(profile_id)
            .map_err(map_read_error)?;
        Ok(extensions
            .into_iter()
            .map(|extension| PathBuf::from(extension.install_path))
            .filter(|path| path.is_dir())
            .collect())
    }

    pub fn assign_defaults_to_profile(&self, profile_id: &str) -> AppResult<()> {
        self.db()
            .assign_default_extensions_to_profile(profile_id)
            .map_err(map_write_error)
    }
}

#[derive(Debug, Deserialize)]
struct ExtensionManifest {
    name: Option<String>,
    version: Option<String>,
}

fn map_browser_extension(
    db: &Database,
    record: BrowserExtensionRecord,
) -> AppResult<BrowserExtension> {
    let profile_ids = db
        .list_extension_profile_ids(&record.id)
        .map_err(map_read_error)?;
    Ok(BrowserExtension {
        id: record.id,
        name: record.name,
        source: record.source,
        store_url: record.store_url,
        store_id: record.store_id,
        version: record.version,
        install_path: record.install_path,
        default_enabled: record.default_enabled,
        profile_ids,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn parse_chrome_store_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let candidate = trimmed
        .split("/detail/")
        .nth(1)
        .and_then(|rest| rest.split('/').find_map(chrome_extension_id_part))
        .or_else(|| trimmed.split('/').find_map(chrome_extension_id_part))
        .or_else(|| is_chrome_extension_id(trimmed).then_some(trimmed))?;
    Some(candidate.to_string())
}

fn chrome_extension_id_part(value: &str) -> Option<&str> {
    let candidate = value.split(['?', '#']).next().unwrap_or(value).trim();
    is_chrome_extension_id(candidate).then_some(candidate)
}

fn is_chrome_extension_id(value: &str) -> bool {
    value.len() == 32 && value.chars().all(|ch| matches!(ch, 'a'..='p'))
}

fn download_chrome_extension(store_id: &str) -> Result<Vec<u8>, veil_runtime::InstallError> {
    let url = format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=120.0&acceptformat=crx2,crx3&x=id%3D{store_id}%26installsource%3Dondemand%26uc"
    );
    veil_runtime::download(&url)
}

fn extract_crx_payload(bytes: &[u8], destination: &Path) -> io::Result<()> {
    if bytes.len() < 16 || &bytes[0..4] != b"Cr24" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid crx"));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("slice length"));
    let zip_offset = match version {
        2 => {
            let public_key_len =
                u32::from_le_bytes(bytes[8..12].try_into().expect("slice length")) as usize;
            let signature_len =
                u32::from_le_bytes(bytes[12..16].try_into().expect("slice length")) as usize;
            16 + public_key_len + signature_len
        }
        3 => {
            let header_len =
                u32::from_le_bytes(bytes[8..12].try_into().expect("slice length")) as usize;
            12 + header_len
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported crx",
            ))
        }
    };
    if zip_offset >= bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing zip payload",
        ));
    }
    let staging = tempfile::NamedTempFile::new()?;
    fs::write(staging.path(), &bytes[zip_offset..])?;
    extract_zip_safely(staging.path(), destination)
}

fn extract_zip_safely(zip_path: &Path, destination: &Path) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    fs::create_dir_all(destination)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unsafe zip entry"))?
            .to_path_buf();
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

fn copy_dir_safely(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_safely(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn read_extension_manifest(install_dir: &Path) -> AppResult<ExtensionManifest> {
    let manifest_path = install_dir.join("manifest.json");
    let mut body = String::new();
    File::open(&manifest_path)
        .and_then(|mut file| file.read_to_string(&mut body))
        .map_err(|_| AppError::from(ErrorCode::ExtensionInvalid))?;
    serde_json::from_str(&body).map_err(|_| AppError::from(ErrorCode::ExtensionInvalid))
}

fn remove_dir_if_under(root: &Path, target: &Path) -> io::Result<()> {
    if target.exists() && target.starts_with(root) {
        fs::remove_dir_all(target)?;
    }
    Ok(())
}

fn dedupe_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn map_read_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ExtensionNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseReadFailed.into()
        }
    }
}

fn map_write_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ExtensionNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseWriteFailed.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chrome_store_ids() {
        assert_eq!(
            parse_chrome_store_id(
                "https://chromewebstore.google.com/detail/name/cjpalhdlnbpafiamejdnhcphjbkeiagm"
            ),
            Some("cjpalhdlnbpafiamejdnhcphjbkeiagm".to_string())
        );
        assert_eq!(
            parse_chrome_store_id("cjpalhdlnbpafiamejdnhcphjbkeiagm"),
            Some("cjpalhdlnbpafiamejdnhcphjbkeiagm".to_string())
        );
        assert_eq!(
            parse_chrome_store_id("https://chromewebstore.google.com/detail/name/cjpalhdlnbpafiamejdnhcphjbkeiagm?hl=en"),
            Some("cjpalhdlnbpafiamejdnhcphjbkeiagm".to_string())
        );
        assert_eq!(parse_chrome_store_id("not-an-extension"), None);
    }
}
