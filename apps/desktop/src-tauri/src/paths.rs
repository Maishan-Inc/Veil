use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Name of the bootstrap pointer file, written in the **default** data directory.
const FILE_NAME: &str = "paths.json";

/// User-chosen locations for the two things Veil is allowed to move.
///
/// This file is the one piece of state that cannot itself be relocated: it answers "where did the
/// user put the data directory", so it has to live at a path Veil can find without already knowing
/// the answer — the OS-default data directory. Everything it points at may be somewhere else.
///
/// Both fields are absent by default, which means "use the resolution rules". A path recorded here
/// is a deliberate user choice, so it wins over the defaults but never over `VEIL_RUNTIME_ROOT` —
/// an environment override exists precisely to get out of a broken persisted state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VeilPaths {
    /// Where profiles, the database, the keystore and extensions live.
    pub data_dir: Option<PathBuf>,
    /// Where engine runtimes are installed.
    pub engine_root: Option<PathBuf>,
    /// A data-directory move the user asked for, to be performed at the next startup.
    ///
    /// The data directory cannot be moved while the app is running: the SQLCipher database is
    /// open, and Windows will not move a file that has a handle on it. Closing the database,
    /// moving, and reopening would leave the app to recover from a failure with no database to
    /// report it through. Recording the intent and doing the move *before* anything is opened is
    /// both simpler and the only ordering where a failure changes nothing.
    pub pending_data_dir: Option<PathBuf>,
}

impl VeilPaths {
    /// Read the pointer file from `default_data_dir`. A missing or unreadable file yields the
    /// defaults: this runs before anything else at startup, and refusing to boot because a
    /// hand-edited JSON file has a stray comma would strand the user with no UI to fix it in.
    pub fn load(default_data_dir: &Path) -> Self {
        let path = default_data_dir.join(FILE_NAME);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write the pointer file, creating the default data directory if needed.
    ///
    /// Written to a temporary file and renamed, so a crash mid-write cannot leave a truncated file
    /// that would silently reset both locations on the next start.
    pub fn save(&self, default_data_dir: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(default_data_dir)?;
        let final_path = default_data_dir.join(FILE_NAME);
        let temp_path = default_data_dir.join(format!("{FILE_NAME}.tmp"));
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        fs::write(&temp_path, bytes)?;
        // Windows `rename` fails if the destination exists, unlike POSIX.
        if final_path.exists() {
            fs::remove_file(&final_path)?;
        }
        fs::rename(&temp_path, &final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_reads_as_defaults() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert_eq!(VeilPaths::load(temp.path()), VeilPaths::default());
    }

    #[test]
    fn round_trips_both_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = VeilPaths {
            data_dir: Some(PathBuf::from("E:/veil-data")),
            engine_root: Some(PathBuf::from("F:/engines")),
            pending_data_dir: None,
        };
        paths.save(temp.path()).expect("save");
        assert_eq!(VeilPaths::load(temp.path()), paths);
    }

    /// Saving twice must not fail on the `rename` — Windows refuses to rename onto an existing
    /// file, so the second save has to remove the old one first.
    #[test]
    fn saving_twice_overwrites() {
        let temp = tempfile::tempdir().expect("temp dir");
        VeilPaths {
            data_dir: Some(PathBuf::from("E:/one")),
            engine_root: None,
            pending_data_dir: None,
        }
        .save(temp.path())
        .expect("first save");
        let second = VeilPaths {
            data_dir: Some(PathBuf::from("E:/two")),
            engine_root: None,
            pending_data_dir: Some(PathBuf::from("E:/three")),
        };
        second.save(temp.path()).expect("second save");
        assert_eq!(VeilPaths::load(temp.path()), second);
    }

    /// A corrupt file must not stop the app from starting; it falls back to the defaults, and the
    /// UI can then write a valid one.
    #[test]
    fn a_corrupt_file_reads_as_defaults() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join(FILE_NAME), b"{ not json").expect("write");
        assert_eq!(VeilPaths::load(temp.path()), VeilPaths::default());
    }
}
