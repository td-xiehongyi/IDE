use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Move,
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum OperationDraftItem {
    Move {
        source_path: String,
        destination_directory: String,
    },
    Rename {
        source_path: String,
        new_name: String,
    },
    AiOrganize {
        source_path: String,
        category_id: String,
        new_name: String,
        content_fingerprint: String,
    },
    AiRename {
        source_path: String,
        new_name: String,
        content_fingerprint: String,
    },
}

impl OperationDraftItem {
    pub fn source_path(&self) -> &str {
        match self {
            Self::Move { source_path, .. }
            | Self::Rename { source_path, .. }
            | Self::AiOrganize { source_path, .. }
            | Self::AiRename { source_path, .. } => source_path,
        }
    }

    pub fn operation_type(&self) -> OperationType {
        match self {
            Self::Move { .. } | Self::AiOrganize { .. } => OperationType::Move,
            Self::Rename { .. } | Self::AiRename { .. } => OperationType::Rename,
        }
    }

    pub fn content_fingerprint(&self) -> Option<&str> {
        match self {
            Self::AiOrganize {
                content_fingerprint,
                ..
            }
            | Self::AiRename {
                content_fingerprint,
                ..
            } => Some(content_fingerprint),
            Self::Move { .. } | Self::Rename { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDraft {
    pub root_path: String,
    pub items: Vec<OperationDraftItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub kind: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub file_identity: Option<String>,
    pub volume_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationValidationStatus {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPreviewItem {
    pub index: usize,
    pub operation: OperationType,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub status: OperationValidationStatus,
    pub reason: Option<String>,
    pub snapshot: Option<FileSnapshot>,
    pub content_fingerprint: Option<String>,
    pub will_create_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPreview {
    pub can_confirm: bool,
    pub items: Vec<OperationPreviewItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationResultStatus {
    Succeeded,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationResultItem {
    pub index: usize,
    pub operation: OperationType,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub status: OperationResultStatus,
    pub reason: Option<String>,
    pub history_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationBatchResult {
    pub batch_id: String,
    pub items: Vec<OperationResultItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryAction {
    Execute,
    Undo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UndoStatus {
    Available,
    Unavailable,
    Undone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationHistoryItem {
    pub id: i64,
    pub batch_id: String,
    pub action: HistoryAction,
    pub operation: OperationType,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub status: OperationResultStatus,
    pub reason: Option<String>,
    pub created_at: String,
    pub undo_status: UndoStatus,
    pub undo_reason: Option<String>,
}
