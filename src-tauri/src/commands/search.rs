use std::path::Path;

use tauri::{AppHandle, Manager, Runtime};

use crate::models::search::{
    ExplorerChildrenQuery, ExplorerNode, IndexedFileQuery, SearchQuery, SearchResult,
};
use crate::services::{path_policy, search};
use crate::storage::{app_paths, database};

use super::AppError;

#[tauri::command]
pub fn search_files<R: Runtime>(
    app: AppHandle<R>,
    mut query: SearchQuery,
) -> Result<SearchResult, AppError> {
    let root = path_policy::normalize_root(Path::new(&query.root_path))?;
    query.root_path = root.to_string_lossy().to_string();
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    search::search(&connection, &query).map_err(AppError::from)
}

#[tauri::command]
pub fn list_explorer_children<R: Runtime>(
    app: AppHandle<R>,
    mut query: ExplorerChildrenQuery,
) -> Result<Vec<ExplorerNode>, AppError> {
    let root = path_policy::normalize_root(Path::new(&query.root_path))?;
    query.root_path = root.to_string_lossy().to_string();
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    search::explorer_children(&connection, &query).map_err(AppError::from)
}

#[tauri::command]
pub fn resolve_indexed_file<R: Runtime>(
    app: AppHandle<R>,
    mut query: IndexedFileQuery,
) -> Result<Option<ExplorerNode>, AppError> {
    let root = path_policy::normalize_root(Path::new(&query.root_path))?;
    query.root_path = root.to_string_lossy().to_string();
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let connection = database::open_database(&database_path).map_err(|error| error.to_string())?;
    search::indexed_file(&connection, &query).map_err(AppError::from)
}
