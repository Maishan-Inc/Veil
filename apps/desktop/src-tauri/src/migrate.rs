use std::fs;
use std::path::{Component, Path, PathBuf};

/// Why a move was refused or failed. Deliberately coarse: the user's next action is the same for
/// every I/O failure, and the interesting cases are the two that are *preventable*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationError {
    /// The destination already holds something. Merging two roots would mean reconciling two
    /// registries, and silently overwriting is worse than asking for an empty folder.
    TargetNotEmpty,
    /// One path is inside the other. `fs::rename` would either fail or, on the copy path, recurse
    /// into its own output.
    NestedPaths,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MigrationSummary {
    pub files: u64,
    pub bytes: u64,
    /// True when the whole tree was renamed (same volume) rather than copied byte by byte.
    pub renamed: bool,
}

/// Check the two things that make a move impossible, without performing it.
///
/// Split out because a data-directory move is only *requested* while the app runs and performed at
/// the next startup — the user has to be told "that folder is not empty" now, not after a restart.
pub fn validate_target(from: &Path, to: &Path) -> Result<(), MigrationError> {
    if is_same_or_nested(from, to) {
        return Err(MigrationError::NestedPaths);
    }
    if !dir_is_empty(to)? {
        return Err(MigrationError::TargetNotEmpty);
    }
    Ok(())
}

/// Move everything under `from` to `to`.
///
/// Order of operations is chosen so that a failure never destroys data: the source is only removed
/// after every byte has been written to the destination *and* the destination has been re-counted
/// and matched against the source census. A crash between those steps leaves two copies, which is
/// recoverable; the reverse would not be.
///
/// A missing or empty source is a success, not an error — switching to a new engine root before
/// any engine is installed is a normal thing to do.
pub fn move_directory(from: &Path, to: &Path) -> Result<MigrationSummary, MigrationError> {
    validate_target(from, to)?;
    let source = census(from)?;
    if source.files == 0 && !from.exists() {
        fs::create_dir_all(to).map_err(|_| MigrationError::Io)?;
        return Ok(MigrationSummary::default());
    }

    // Same-volume fast path. `rename` refuses an existing destination on Windows even when it is
    // empty, so the empty directory the user may have just created has to go first.
    if to.exists() {
        let _ = fs::remove_dir(to);
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|_| MigrationError::Io)?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(MigrationSummary {
            files: source.files,
            bytes: source.bytes,
            renamed: true,
        });
    }

    copy_tree(from, to).map_err(|_| MigrationError::Io)?;
    let copied = census(to)?;
    if copied != source {
        return Err(MigrationError::Io);
    }
    fs::remove_dir_all(from).map_err(|_| MigrationError::Io)?;
    Ok(MigrationSummary {
        files: source.files,
        bytes: source.bytes,
        renamed: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Census {
    files: u64,
    bytes: u64,
}

/// Count files and bytes under `root`. A missing directory counts as empty.
fn census(root: &Path) -> Result<Census, MigrationError> {
    let mut total = Census::default();
    if !root.exists() {
        return Ok(total);
    }
    let entries = fs::read_dir(root).map_err(|_| MigrationError::Io)?;
    for entry in entries {
        let entry = entry.map_err(|_| MigrationError::Io)?;
        let file_type = entry.file_type().map_err(|_| MigrationError::Io)?;
        if file_type.is_dir() {
            let nested = census(&entry.path())?;
            total.files += nested.files;
            total.bytes += nested.bytes;
        } else {
            let meta = entry.metadata().map_err(|_| MigrationError::Io)?;
            total.files += 1;
            total.bytes += meta.len();
        }
    }
    Ok(total)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// True when `to` does not exist, or exists as a directory with no entries.
fn dir_is_empty(to: &Path) -> Result<bool, MigrationError> {
    if !to.exists() {
        return Ok(true);
    }
    if !to.is_dir() {
        return Ok(false);
    }
    let mut entries = fs::read_dir(to).map_err(|_| MigrationError::Io)?;
    Ok(entries.next().is_none())
}

/// True when the two paths are the same directory, or one contains the other.
///
/// Compared on normalized components rather than canonicalized paths, because the destination
/// usually does not exist yet and `canonicalize` needs a real file. Windows paths are compared
/// case-insensitively, which is the behaviour of the filesystem this ships on.
fn is_same_or_nested(a: &Path, b: &Path) -> bool {
    let left = normalize(a);
    let right = normalize(b);
    left.starts_with(&right) || right.starts_with(&left)
}

fn normalize(path: &Path) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }

    #[test]
    fn moves_a_tree_and_leaves_nothing_behind() {
        let temp = tempfile::tempdir().expect("temp");
        let from = temp.path().join("old");
        let to = temp.path().join("new");
        write(&from.join("registry.json"), b"{}");
        write(&from.join("versions/151/chrome.exe"), b"MZ_binary");

        let summary = move_directory(&from, &to).expect("move");
        assert_eq!(summary.files, 2);
        assert_eq!(summary.bytes, 2 + 9);
        assert!(!from.exists(), "source must be gone");
        assert_eq!(
            fs::read(to.join("versions/151/chrome.exe")).expect("moved file"),
            b"MZ_binary"
        );
    }

    #[test]
    fn a_missing_source_is_a_success_and_creates_the_target() {
        let temp = tempfile::tempdir().expect("temp");
        let to = temp.path().join("new");
        let summary = move_directory(&temp.path().join("absent"), &to).expect("move");
        assert_eq!(summary, MigrationSummary::default());
        assert!(to.is_dir());
    }

    #[test]
    fn an_empty_target_directory_is_accepted() {
        let temp = tempfile::tempdir().expect("temp");
        let from = temp.path().join("old");
        let to = temp.path().join("new");
        write(&from.join("a.txt"), b"a");
        fs::create_dir_all(&to).expect("target");
        assert!(move_directory(&from, &to).is_ok());
    }

    #[test]
    fn refuses_a_target_that_holds_anything() {
        let temp = tempfile::tempdir().expect("temp");
        let from = temp.path().join("old");
        let to = temp.path().join("new");
        write(&from.join("a.txt"), b"a");
        write(&to.join("keep-me.txt"), b"existing");
        assert_eq!(
            move_directory(&from, &to),
            Err(MigrationError::TargetNotEmpty)
        );
        assert!(from.join("a.txt").exists(), "source must be untouched");
        assert_eq!(
            fs::read(to.join("keep-me.txt")).expect("kept"),
            b"existing",
            "destination must be untouched"
        );
    }

    #[test]
    fn refuses_nested_and_identical_paths() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("engines");
        write(&root.join("a.txt"), b"a");
        for target in [root.clone(), root.join("inner"), temp.path().to_path_buf()] {
            assert_eq!(
                move_directory(&root, &target),
                Err(MigrationError::NestedPaths),
                "{target:?} must be refused"
            );
        }
    }

    /// Windows paths differ only in case all the time (`C:\Users` vs `c:\users`); a case-sensitive
    /// comparison would let the user move a directory into itself.
    #[test]
    fn nesting_is_detected_case_insensitively() {
        assert!(is_same_or_nested(
            Path::new("C:/Apps/Veil/runtime"),
            Path::new("c:/apps/veil/RUNTIME/versions")
        ));
        assert!(!is_same_or_nested(
            Path::new("C:/Apps/Veil/runtime"),
            Path::new("D:/engines")
        ));
    }
}
