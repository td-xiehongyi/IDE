use std::path::{Path, PathBuf};

use crate::models::file_entry::{EntryKind, FileEntry};
use crate::models::scan::{ScanError, ScanOutput};

use super::path_policy::{is_ignored_name, normalize_root};

pub fn scan_directory(root: &Path) -> Result<ScanOutput, String> {
    let normalized_root = normalize_root(root)?;
    let mut output = ScanOutput {
        entries: Vec::new(),
        errors: Vec::new(),
        ignored: 0,
    };
    visit_directory(&normalized_root, &mut output);
    Ok(output)
}

fn visit_directory(directory: &Path, output: &mut ScanOutput) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            record_error(output, directory, "directory_read", error.to_string());
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                record_error(output, directory, "entry_read", error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                record_error(output, &path, "metadata", error.to_string());
                continue;
            }
        };

        if is_ignored_name(&name, file_type.is_dir()) {
            output.ignored += 1;
            continue;
        }

        if file_type.is_dir() {
            match directory_entry(&path, &name) {
                Ok(entry) => output.entries.push(entry),
                Err(error) => record_error(output, &path, "metadata", error),
            }
            visit_directory(&path, output);
        } else if file_type.is_symlink() {
            match link_entry(&path, &name, EntryKind::Symlink) {
                Ok(entry) => output.entries.push(entry),
                Err(error) => record_error(output, &path, "link_metadata", error),
            }
        } else if file_type.is_file() {
            match file_entry(&path, &name, EntryKind::File) {
                Ok(entry) => output.entries.push(entry),
                Err(error) => record_error(output, &path, "metadata", error),
            }
        }
    }
}

fn file_entry(path: &Path, name: &str, kind: EntryKind) -> Result<FileEntry, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(FileEntry {
        normalized_path: std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)),
        name: name.to_string(),
        extension: Path::new(name)
            .extension()
            .map(|value| value.to_string_lossy().to_string()),
        kind,
        size: metadata.len(),
        modified_ms: metadata.modified().ok().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_millis() as i64)
        }),
        file_identity: None,
    })
}

fn directory_entry(path: &Path, name: &str) -> Result<FileEntry, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(FileEntry {
        normalized_path: std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)),
        name: name.to_string(),
        extension: None,
        kind: EntryKind::Directory,
        size: 0,
        modified_ms: metadata.modified().ok().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_millis() as i64)
        }),
        file_identity: None,
    })
}

pub fn entry_from_path(path: &Path) -> Option<FileEntry> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let file_type = std::fs::symlink_metadata(path).ok()?.file_type();
    if file_type.is_symlink() {
        link_entry(path, &name, EntryKind::Symlink).ok()
    } else if file_type.is_dir() {
        directory_entry(path, &name).ok()
    } else if file_type.is_file() {
        file_entry(path, &name, EntryKind::File).ok()
    } else {
        None
    }
}

fn link_entry(path: &Path, name: &str, kind: EntryKind) -> Result<FileEntry, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    Ok(FileEntry {
        normalized_path: PathBuf::from(path),
        name: name.to_string(),
        extension: Path::new(name)
            .extension()
            .map(|value| value.to_string_lossy().to_string()),
        kind,
        size: metadata.len(),
        modified_ms: metadata.modified().ok().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_millis() as i64)
        }),
        file_identity: None,
    })
}

fn record_error(output: &mut ScanOutput, path: &Path, kind: &str, message: String) {
    output.errors.push(ScanError {
        path: path.to_string_lossy().to_string(),
        kind: kind.to_string(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("ai-file-organizer-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn scans_nested_files_and_ignores_generated_directories() {
        let root = temp_dir("scan");
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("nested/note.txt"), "hello").unwrap();
        fs::write(root.join("node_modules/ignored.txt"), "ignored").unwrap();

        let result = scan_directory(&root).unwrap();

        assert!(result.entries.iter().any(|entry| entry.name == "nested"));
        assert!(result.entries.iter().any(|entry| entry.name == "note.txt"));
        assert_eq!(result.ignored, 1);
        assert!(result.errors.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn records_symlink_without_following_it() {
        let root = temp_dir("symlink");
        let outside = temp_dir("outside");
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        let link_result = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&outside, root.join("linked"))
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_dir(&outside, root.join("linked"))
            }
        };
        if link_result.is_err() {
            let _ = fs::remove_dir_all(root);
            let _ = fs::remove_dir_all(outside);
            return;
        }

        let result = scan_directory(&root).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, EntryKind::Symlink);
        assert!(
            !result
                .entries
                .iter()
                .any(|entry| entry.name == "secret.txt")
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }
}
