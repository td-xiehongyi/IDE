//! Narrow Tauri command entry points live here.

use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::models::scan::{IndexStatus, ScanMode, ScanProgress, ScanSummary};
use crate::services::{path_policy, scanner, watcher};
use crate::storage::{app_paths, database, file_repository};

pub mod ai;
pub mod operations;
pub mod search;

#[derive(Debug, serde::Serialize)]
pub struct AppError {
    pub message: String,
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

#[tauri::command]
pub fn scan_directory<R: Runtime>(
    app: AppHandle<R>,
    root_path: String,
    mode: ScanMode,
) -> Result<ScanSummary, AppError> {
    let root = path_policy::normalize_root(Path::new(&root_path))?;
    let progress =
        |phase: &str, visited: usize, indexed: usize, errors: usize, current: Option<String>| {
            let _ = app.emit(
                "files://scan-progress",
                ScanProgress {
                    root_path: root.to_string_lossy().to_string(),
                    visited_entries: visited,
                    indexed_entries: indexed,
                    error_count: errors,
                    current_path: current,
                    phase: phase.to_string(),
                },
            );
        };
    progress("scanning", 0, 0, 0, None);
    let output = scanner::scan_directory(&root)?;
    progress(
        "persisting",
        output.entries.len(),
        output.entries.len(),
        output.errors.len(),
        None,
    );
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let database_path = app_paths::database_path(&app_data_dir);
    let mut connection =
        database::open_database(&database_path).map_err(|error| error.to_string())?;
    let now = now_string();
    let scan_root = file_repository::ScanRoot {
        path: root_path,
        normalized_path: root.to_string_lossy().to_string(),
        created_at: now.clone(),
        last_scan_at: Some(now.clone()),
    };
    let root_id = file_repository::upsert_scan_root(&connection, &scan_root)
        .map_err(|error| error.to_string())?;
    let delta = file_repository::replace_entries_for_root(
        &mut connection,
        root_id,
        &output.entries,
        &output.errors,
        &now,
    )
    .map_err(|error| error.to_string())?;
    let files = output
        .entries
        .iter()
        .filter(|entry| matches!(entry.kind, crate::models::file_entry::EntryKind::File))
        .count() as i64;
    let directories = output
        .entries
        .iter()
        .filter(|entry| matches!(entry.kind, crate::models::file_entry::EntryKind::Directory))
        .count() as i64;
    let links = output.entries.len() as i64 - files - directories;
    let summary = ScanSummary {
        root_path: root.to_string_lossy().to_string(),
        mode,
        indexed_files: files,
        indexed_directories: directories,
        indexed_links: links,
        added: delta.added,
        updated: delta.updated,
        removed: delta.removed,
        ignored: output.ignored as i64,
        errors: output.errors.len() as i64,
        completed_at: now,
    };
    if let Err(error) = app
        .state::<watcher::WatcherState>()
        .replace(app.clone(), &root)
    {
        let _ = app.emit("files://watcher-error", error);
    }
    progress(
        "completed",
        output.entries.len(),
        output.entries.len(),
        output.errors.len(),
        None,
    );
    Ok(summary)
}

#[tauri::command]
pub fn get_index_status<R: Runtime>(
    app: AppHandle<R>,
    root_path: String,
) -> Result<IndexStatus, AppError> {
    let root = path_policy::normalize_root(Path::new(&root_path))?;
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    let stored = file_repository::read_index_status(&connection, &root)
        .map_err(|error| error.to_string())?;
    let (indexed_entries, last_scan_at) = stored.unwrap_or((0, None));
    Ok(IndexStatus {
        root_path: root.to_string_lossy().to_string(),
        indexed_entries,
        last_scan_at,
        state: if indexed_entries == 0 {
            "empty"
        } else {
            "ready"
        }
        .into(),
    })
}

#[tauri::command]
pub fn restore_recent_index<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<IndexStatus>, AppError> {
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    let Some((stored_path, indexed_entries, last_scan_at)) =
        file_repository::read_latest_index_status(&connection)
            .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let root = match path_policy::normalize_root(Path::new(&stored_path)) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    if root.to_string_lossy() != stored_path {
        return Ok(None);
    }
    app.state::<watcher::WatcherState>()
        .replace(app.clone(), &root)?;
    Ok(Some(IndexStatus {
        root_path: stored_path,
        indexed_entries,
        last_scan_at,
        state: if indexed_entries == 0 {
            "empty"
        } else {
            "ready"
        }
        .into(),
    }))
}

#[tauri::command]
pub fn rebuild_index<R: Runtime>(app: AppHandle<R>) -> Result<(), AppError> {
    app.state::<watcher::WatcherState>().stop()?;
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    file_repository::reset_file_index(&connection).map_err(|error| error.to_string())?;
    let _ = app.emit("files://index-changed", ());
    Ok(())
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
