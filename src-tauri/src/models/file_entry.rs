use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Junction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub normalized_path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub kind: EntryKind,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub file_identity: Option<String>,
}
