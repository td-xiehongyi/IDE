use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::AppError;
use crate::models::operation::{
    HistoryAction, OperationBatchResult, OperationDraft, OperationHistoryItem, OperationPreview,
    OperationResultStatus, UndoStatus,
};
use crate::services::{operation_executor, operation_validator, undo_service};
use crate::services::{path_policy, watcher};
use crate::storage::{app_paths, database, operation_repository};

#[derive(Debug, Clone, Serialize)]
pub struct OperationPreviewResponse {
    pub can_confirm: bool,
    pub items: Vec<crate::models::operation::OperationPreviewItem>,
    pub plan_id: Option<String>,
    pub expires_at: Option<String>,
}

#[tauri::command]
pub fn preview_operations<R: Runtime>(
    app: AppHandle<R>,
    draft: OperationDraft,
) -> Result<OperationPreviewResponse, AppError> {
    let normalized_root = path_policy::normalize_root(std::path::Path::new(&draft.root_path))?;
    if !app
        .state::<watcher::WatcherState>()
        .is_current_root(&normalized_root)?
    {
        return Err("只能操作当前授权根目录内的文件。".to_string().into());
    }
    let preview = operation_validator::validate_draft(&draft)?;
    if !preview.can_confirm {
        return Ok(response_from_preview(preview, None, None));
    }
    let token = app
        .state::<crate::services::plan_store::PlanStore>()
        .create(preview.clone())?;
    Ok(response_from_preview(
        preview,
        Some(token.plan_id),
        Some(format_system_time(token.expires_at)),
    ))
}

#[tauri::command]
pub fn cancel_operation_plan<R: Runtime>(
    app: AppHandle<R>,
    plan_id: String,
) -> Result<(), AppError> {
    app.state::<crate::services::plan_store::PlanStore>()
        .cancel(&plan_id)?;
    Ok(())
}

#[tauri::command]
pub fn execute_operation_plan<R: Runtime>(
    app: AppHandle<R>,
    plan_id: String,
) -> Result<OperationBatchResult, AppError> {
    let plan = app
        .state::<crate::services::plan_store::PlanStore>()
        .consume(&plan_id, SystemTime::now())?;
    let connection = open_database(&app)?;
    let result = operation_executor::execute_plan(&connection, &plan)?;
    let _ = app.emit("files://index-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn get_operation_history<R: Runtime>(
    app: AppHandle<R>,
    limit: i64,
    offset: i64,
) -> Result<Vec<OperationHistoryItem>, AppError> {
    let connection = open_database(&app)?;
    let mut history =
        operation_repository::read_history(&connection, limit.clamp(1, 100), offset.max(0))
            .map_err(|error| error.to_string())?;
    for item in &mut history {
        update_undo_status(&connection, item)?;
    }
    Ok(history)
}

#[tauri::command]
pub fn undo_operation<R: Runtime>(
    app: AppHandle<R>,
    history_id: i64,
) -> Result<crate::models::operation::OperationResultItem, AppError> {
    let connection = open_database(&app)?;
    let result = undo_service::undo_history(&connection, history_id)?;
    let _ = app.emit("files://index-changed", ());
    Ok(result)
}

fn response_from_preview(
    preview: OperationPreview,
    plan_id: Option<String>,
    expires_at: Option<String>,
) -> OperationPreviewResponse {
    OperationPreviewResponse {
        can_confirm: preview.can_confirm,
        items: preview.items,
        plan_id,
        expires_at,
    }
}

fn open_database<R: Runtime>(app: &AppHandle<R>) -> Result<rusqlite::Connection, AppError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    database::open_database(&app_paths::database_path(&app_data_dir))
        .map_err(|error| error.to_string().into())
}

fn update_undo_status(
    connection: &rusqlite::Connection,
    item: &mut OperationHistoryItem,
) -> Result<(), AppError> {
    if item.action != HistoryAction::Execute || item.status != OperationResultStatus::Succeeded {
        item.undo_status = UndoStatus::Unavailable;
        item.undo_reason = Some("只有成功的原始操作可以撤销。".into());
        return Ok(());
    }
    if operation_repository::has_successful_undo(connection, item.id)
        .map_err(|error| error.to_string())?
    {
        item.undo_status = UndoStatus::Undone;
        item.undo_reason = None;
        return Ok(());
    }
    let Some((_, Some(snapshot), _)) =
        operation_repository::read_history_record(connection, item.id)
            .map_err(|error| error.to_string())?
    else {
        item.undo_status = UndoStatus::Unavailable;
        item.undo_reason = Some("缺少文件身份快照。".into());
        return Ok(());
    };
    if fs::symlink_metadata(&item.source_path).is_ok() {
        item.undo_status = UndoStatus::Unavailable;
        item.undo_reason = Some("原始路径已被占用。".into());
        return Ok(());
    }
    if fs::symlink_metadata(&item.target_path).is_err() {
        item.undo_status = UndoStatus::Unavailable;
        item.undo_reason = Some("当前目标文件不存在。".into());
        return Ok(());
    }
    if let Err(reason) =
        crate::services::file_identity::snapshot_matches(&item.target_path, &snapshot)
    {
        item.undo_status = UndoStatus::Unavailable;
        item.undo_reason = Some(reason);
        return Ok(());
    }
    item.undo_status = UndoStatus::Available;
    item.undo_reason = None;
    Ok(())
}

fn format_system_time(value: SystemTime) -> String {
    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
