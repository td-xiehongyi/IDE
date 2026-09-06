use serde::{Deserialize, Serialize};

use super::file_entry::FileEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Incremental,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanError {
    pub path: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanProgress {
    pub root_path: String,
    pub visited_entries: usize,
    pub indexed_entries: usize,
    pub error_count: usize,
    pub current_path: Option<String>,
    pub phase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSummary {
    pub root_path: String,
    pub mode: ScanMode,
    pub indexed_files: i64,
    pub indexed_directories: i64,
    pub indexed_links: i64,
    pub added: i64,
    pub updated: i64,
    pub removed: i64,
    pub ignored: i64,
    pub errors: i64,
    pub completed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStatus {
    pub root_path: String,
    pub indexed_entries: i64,
    pub last_scan_at: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutput {
    pub entries: Vec<FileEntry>,
    pub errors: Vec<ScanError>,
    pub ignored: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDelta {
    pub added: i64,
    pub updated: i64,
    pub removed: i64,
}
