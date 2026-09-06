use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::models::operation::{
    HistoryAction, OperationBatchResult, OperationHistoryItem, OperationResultItem,
    OperationResultStatus, OperationType, UndoStatus,
};
use crate::storage::operation_repository;

use super::file_identity::snapshot_matches;
use super::path_policy::{category_directory, normalize_root};
use super::plan_store::ValidatedPlan;

pub fn execute_plan(
    connection: &Connection,
    plan: &ValidatedPlan,
) -> Result<OperationBatchResult, String> {
    let batch_id = format!("batch-{}", plan.plan_id);
    let mut items = Vec::with_capacity(plan.items.len());
    let mut stopped = false;

    for item in &plan.items {
        if stopped {
            items.push(record_result(
                connection,
                &batch_id,
                item.index,
                item.operation.clone(),
                item.source_path.clone(),
                item.target_path.clone(),
                OperationResultStatus::NotExecuted,
                Some("前序项目执行失败，当前项目未执行。".into()),
                None,
            )?);
            continue;
        }

        let outcome = execute_item(item);
        let result = match outcome {
            Ok(()) => record_result(
                connection,
                &batch_id,
                item.index,
                item.operation.clone(),
                item.source_path.clone(),
                item.target_path.clone(),
                OperationResultStatus::Succeeded,
                None,
                item.snapshot.as_ref(),
            )?,
            Err(reason) => {
                stopped = true;
                record_result(
                    connection,
                    &batch_id,
                    item.index,
                    item.operation.clone(),
                    item.source_path.clone(),
                    item.target_path.clone(),
                    OperationResultStatus::Failed,
                    Some(reason),
                    None,
                )?
            }
        };
        items.push(result);
    }

    Ok(OperationBatchResult { batch_id, items })
}

fn execute_item(item: &crate::models::operation::OperationPreviewItem) -> Result<(), String> {
    let snapshot = item
        .snapshot
        .as_ref()
        .ok_or_else(|| "操作项目缺少文件快照。".to_string())?;
    snapshot_matches(&item.source_path, snapshot)?;
    if let Some(expected) = item.content_fingerprint.as_deref() {
        let actual = super::content_extractor::fingerprint_file(&item.source_path)?;
        if actual != expected {
            return Err("执行前内容指纹发生变化，AI 建议已失效。".into());
        }
    }
    if fs::symlink_metadata(&item.target_path).is_ok() {
        return Err("执行前目标路径已出现，拒绝覆盖。".into());
    }
    let parent = item
        .target_path
        .parent()
        .ok_or_else(|| "目标父目录不可用。".to_string())?;
    if item.will_create_directory {
        create_category_directory(parent)?;
    }
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.file_type().is_dir() {
        return Err("执行前目标目录已不可用。".into());
    }
    fs::rename(&item.source_path, &item.target_path).map_err(|error| error.to_string())
}

fn create_category_directory(directory: &std::path::Path) -> Result<(), String> {
    let root_input = directory
        .parent()
        .ok_or_else(|| "分类目标目录的授权根目录不可用。".to_string())?;
    let root = normalize_root(root_input)?;
    let category_id = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "分类目标目录名称不可用。".to_string())?;
    let expected = category_directory(&root, category_id)?;
    if expected != directory {
        return Err("分类目标目录不是当前授权根目录下的单层分类目录。".into());
    }
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() => {
            return Err("执行前分类目标目录已被占用，拒绝继续。".into());
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error.to_string());
        }
        Err(_) => {}
    }
    fs::create_dir(directory).map_err(|error| error.to_string())?;
    let metadata = fs::symlink_metadata(directory).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err("新建的分类目标目录不可安全使用。".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_result(
    connection: &Connection,
    batch_id: &str,
    index: usize,
    operation: OperationType,
    source_path: std::path::PathBuf,
    target_path: std::path::PathBuf,
    status: OperationResultStatus,
    reason: Option<String>,
    snapshot: Option<&crate::models::operation::FileSnapshot>,
) -> Result<OperationResultItem, String> {
    let record = OperationHistoryItem {
        id: 0,
        batch_id: batch_id.into(),
        action: HistoryAction::Execute,
        operation: operation.clone(),
        source_path: source_path.clone(),
        target_path: target_path.clone(),
        status: status.clone(),
        reason: reason.clone(),
        created_at: now_string(),
        undo_status: UndoStatus::Unavailable,
        undo_reason: None,
    };
    let history_id = operation_repository::insert_history(connection, &record, snapshot, None)
        .map_err(|error| error.to_string())?;
    Ok(OperationResultItem {
        index,
        operation,
        source_path,
        target_path,
        status,
        reason,
        history_id: Some(history_id),
    })
}

fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
