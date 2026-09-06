use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::models::operation::{
    HistoryAction, OperationHistoryItem, OperationResultItem, OperationResultStatus, UndoStatus,
};
use crate::storage::operation_repository;

use super::file_identity::snapshot_matches;

pub fn undo_history(
    connection: &Connection,
    history_id: i64,
) -> Result<OperationResultItem, String> {
    let (record, snapshot, _) = operation_repository::read_history_record(connection, history_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "操作历史不存在。".to_string())?;
    if record.action != HistoryAction::Execute || record.status != OperationResultStatus::Succeeded
    {
        return Err("只有成功的原始操作可以撤销。".into());
    }
    if operation_repository::has_successful_undo(connection, history_id)
        .map_err(|error| error.to_string())?
    {
        return Err("该操作已经撤销。".into());
    }
    let snapshot = snapshot.ok_or_else(|| "操作历史缺少文件身份快照。".to_string())?;
    if fs::symlink_metadata(&record.source_path).is_ok() {
        return Err("原始路径已被占用，撤销不会覆盖现有内容。".into());
    }
    if !fs::symlink_metadata(&record.target_path)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_file()
    {
        return Err("当前目标不是普通文件，无法安全撤销。".into());
    }
    snapshot_matches(&record.target_path, &snapshot)?;
    let source_parent = record
        .source_path
        .parent()
        .ok_or_else(|| "原始路径的父目录不可用。".to_string())?;
    if !fs::symlink_metadata(source_parent)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_dir()
    {
        return Err("原始父目录不存在，无法安全撤销。".into());
    }
    fs::rename(&record.target_path, &record.source_path).map_err(|error| error.to_string())?;

    let undo_record = OperationHistoryItem {
        id: 0,
        batch_id: record.batch_id.clone(),
        action: HistoryAction::Undo,
        operation: record.operation.clone(),
        source_path: record.target_path.clone(),
        target_path: record.source_path.clone(),
        status: OperationResultStatus::Succeeded,
        reason: None,
        created_at: now_string(),
        undo_status: UndoStatus::Unavailable,
        undo_reason: None,
    };
    let undo_id = operation_repository::insert_history(
        connection,
        &undo_record,
        Some(&snapshot),
        Some(history_id),
    )
    .map_err(|error| error.to_string())?;
    Ok(OperationResultItem {
        index: 0,
        operation: record.operation,
        source_path: record.target_path,
        target_path: record.source_path,
        status: OperationResultStatus::Succeeded,
        reason: None,
        history_id: Some(undo_id),
    })
}

fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
