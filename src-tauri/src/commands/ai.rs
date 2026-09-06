use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::ai::AiProvider;
use crate::ai::ProviderStatus;
use crate::ai::ollama::{DEFAULT_MODEL, DEFAULT_OLLAMA_ENDPOINT, OllamaProvider};
use crate::commands::AppError;
use crate::models::ai::{
    AiAnalysisRecord, AnalysisBatchStatus, AnalysisResultStatus, AnalysisTaskSnapshot, Category,
    CategoryTemplate, TemplateCategory,
};
use crate::models::operation::OperationDraft;
use crate::services::analysis_service;
use crate::services::analysis_task_store::AnalysisTaskStore;
use crate::services::content_extractor::fingerprint_file;
use crate::services::suggestion_review::{self, ReviewAction};
use crate::services::{path_policy, watcher};
use crate::storage::{ai_repository, app_paths, database};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StartAnalysisRequest {
    pub root_path: String,
    pub file_paths: Vec<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StartAnalysisResponse {
    pub batch_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalysisProgress {
    pub batch_id: String,
    pub phase: String,
    pub completed_files: usize,
    pub total_files: usize,
    pub current_path: Option<String>,
    pub error_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReviewAnalysisRequest {
    pub result_id: String,
    pub action: ReviewAction,
    pub suggested_filename: Option<String>,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SaveCategoryTemplateRequest {
    pub id: String,
    pub name: String,
    pub categories: Vec<TemplateCategory>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApplyCategoryTemplateRequest {
    pub root_path: String,
    pub template_id: String,
    pub categories: Vec<Category>,
}

fn emit_analysis_progress<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &AnalysisTaskSnapshot,
    phase: &str,
) {
    let _ = app.emit(
        "ai://analysis-progress",
        AnalysisProgress {
            batch_id: snapshot.batch_id.clone(),
            phase: phase.into(),
            completed_files: snapshot.completed_files,
            total_files: snapshot.total_files,
            current_path: snapshot.current_path.clone(),
            error_count: snapshot.failures.len(),
        },
    );
}

fn delete_batch_results(database_path: &Path, batch_id: &str) {
    if let Ok(connection) = database::open_database(database_path) {
        let _ = ai_repository::delete_batch_results(&connection, batch_id);
    }
}

#[tauri::command]
pub fn get_ai_provider_status(model: Option<String>) -> ProviderStatus {
    let model = configured_model(model);
    match OllamaProvider::new(DEFAULT_OLLAMA_ENDPOINT.into(), model.clone())
        .and_then(|provider| provider.health())
    {
        Ok(status) => status,
        Err(message) => ProviderStatus {
            available: false,
            provider: "ollama".into(),
            model,
            message,
        },
    }
}

#[tauri::command]
pub fn save_ai_categories<R: Runtime>(
    app: AppHandle<R>,
    root_path: String,
    categories: Vec<Category>,
) -> Result<Vec<Category>, AppError> {
    let root = require_current_root(&app, &root_path)?;
    let categories = validate_categories(&root, categories)?;
    let mut connection = open_database(&app)?;
    ai_repository::replace_categories(&mut connection, &root.to_string_lossy(), &categories)
        .map_err(|error| error.to_string())?;
    Ok(categories)
}

#[tauri::command]
pub fn get_ai_categories<R: Runtime>(
    app: AppHandle<R>,
    root_path: String,
) -> Result<Vec<Category>, AppError> {
    let root = require_current_root(&app, &root_path)?;
    ai_repository::read_categories(&open_database(&app)?, &root.to_string_lossy())
        .map_err(|error| error.to_string().into())
}

#[tauri::command]
pub fn get_ai_category_templates<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<CategoryTemplate>, AppError> {
    ai_repository::read_category_templates(&open_database(&app)?)
        .map_err(|error| error.to_string().into())
}

#[tauri::command]
pub fn save_ai_category_template<R: Runtime>(
    app: AppHandle<R>,
    request: SaveCategoryTemplateRequest,
) -> Result<CategoryTemplate, AppError> {
    validate_template_id(&request.id)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err("模板名称不能为空".to_string().into());
    }
    let categories = validate_template_categories(request.categories)?;
    let mut connection = open_database(&app)?;
    ai_repository::upsert_category_template(
        &mut connection,
        &request.id,
        name,
        &categories,
        &now_string(),
    )
    .map_err(|error| error.to_string().into())
}

#[tauri::command]
pub fn delete_ai_category_template<R: Runtime>(
    app: AppHandle<R>,
    template_id: String,
) -> Result<(), AppError> {
    let deleted = ai_repository::delete_category_template(&open_database(&app)?, &template_id)
        .map_err(|error| error.to_string())?;
    if !deleted {
        return Err("分类模板不存在".to_string().into());
    }
    Ok(())
}

#[tauri::command]
pub fn apply_ai_category_template<R: Runtime>(
    app: AppHandle<R>,
    request: ApplyCategoryTemplateRequest,
) -> Result<Vec<Category>, AppError> {
    let root = require_current_root(&app, &request.root_path)?;
    let mut connection = open_database(&app)?;
    let template = ai_repository::read_category_template(&connection, &request.template_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "分类模板不存在".to_string())?;
    let categories = validate_applied_categories(&root, &template, request.categories)?;
    let categories = validate_categories(&root, categories)?;
    ai_repository::replace_categories(&mut connection, &root.to_string_lossy(), &categories)
        .map_err(|error| error.to_string())?;
    ai_repository::bind_root_to_category_template(
        &connection,
        &root.to_string_lossy(),
        &template.id,
        template.version,
    )
    .map_err(|error| error.to_string())?;
    Ok(categories)
}

#[tauri::command]
pub fn delete_ai_category<R: Runtime>(
    app: AppHandle<R>,
    root_path: String,
    category_id: String,
) -> Result<(), AppError> {
    let root = require_current_root(&app, &root_path)?;
    let root_string = root.to_string_lossy().to_string();
    let connection = open_database(&app)?;
    let category = ai_repository::read_categories(&connection, &root_string)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|category| category.id == category_id)
        .ok_or_else(|| "分类配置不存在".to_string())?;
    if ai_repository::count_pending_category_references(&connection, &root_string, &category_id)
        .map_err(|error| error.to_string())?
        > 0
    {
        return Err("该分类仍被待确认的 AI 建议引用，请先处理这些建议"
            .to_string()
            .into());
    }
    let directory =
        path_policy::category_directory_for_category(&root, &category.id, &category.name)?;
    if app
        .state::<crate::services::plan_store::PlanStore>()
        .has_target_directory(&directory)?
    {
        return Err("该分类仍被待确认的操作草案引用，请先处理操作预览"
            .to_string()
            .into());
    }
    if !ai_repository::delete_category(&connection, &root_string, &category_id)
        .map_err(|error| error.to_string())?
    {
        return Err("分类配置不存在".to_string().into());
    }
    Ok(())
}

#[tauri::command]
pub fn start_analysis_batch<R: Runtime>(
    app: AppHandle<R>,
    request: StartAnalysisRequest,
) -> Result<StartAnalysisResponse, AppError> {
    let root = require_current_root(&app, &request.root_path)?;
    if request.file_paths.is_empty() || request.file_paths.len() > 100 {
        return Err("每个分析批次必须包含 1 到 100 个文件".to_string().into());
    }
    let connection = open_database(&app)?;
    let categories = ai_repository::read_categories(&connection, &root.to_string_lossy())
        .map_err(|error| error.to_string())?;
    let template = ai_repository::read_root_category_template(&connection, &root.to_string_lossy())
        .map_err(|error| error.to_string())?;
    if !categories.iter().any(|category| category.enabled) {
        return Err("请先配置至少一个启用的分类".to_string().into());
    }
    let model = configured_model(request.model);
    let provider = OllamaProvider::new(DEFAULT_OLLAMA_ENDPOINT.into(), model)?;
    let health = provider.health()?;
    if !health.available {
        return Err(health.message.into());
    }
    let batch_id = app
        .state::<AnalysisTaskStore>()
        .create(request.file_paths.clone(), SystemTime::now())?;
    let response = StartAnalysisResponse {
        batch_id: batch_id.clone(),
    };
    let database_path = app_paths::database_path(
        &app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?,
    );
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = worker_app.state::<AnalysisTaskStore>();
        if store.mark_running(&batch_id).is_err() {
            if !store.is_cancelled(&batch_id) {
                let _ = store.fail(&batch_id, "分析任务无法启动".into());
                if let Some(snapshot) = store.get(&batch_id) {
                    emit_analysis_progress(&worker_app, &snapshot, "failed");
                }
            }
            return;
        }
        if let Some(snapshot) = store.get(&batch_id) {
            emit_analysis_progress(&worker_app, &snapshot, "processing");
        }
        let progress_app = worker_app.clone();
        let cancel_app = worker_app.clone();
        let progress_batch = batch_id.clone();
        let result = database::open_database(&database_path)
            .map_err(|error| error.to_string())
            .and_then(|connection| {
                analysis_service::analyze_batch(
                    &connection,
                    &batch_id,
                    &root,
                    &request.file_paths,
                    &categories,
                    template
                        .as_ref()
                        .map(|(id, version)| (id.as_str(), *version)),
                    &provider,
                    || {
                        cancel_app
                            .state::<AnalysisTaskStore>()
                            .is_cancelled(&batch_id)
                    },
                    |completed, total, current| {
                        let state = progress_app.state::<AnalysisTaskStore>();
                        let _ = state.update_progress(&progress_batch, completed, current.clone());
                        let failures = state
                            .get(&progress_batch)
                            .map_or(0, |snapshot| snapshot.failures.len());
                        let _ = progress_app.emit(
                            "ai://analysis-progress",
                            AnalysisProgress {
                                batch_id: progress_batch.clone(),
                                phase: if current.is_some() {
                                    "analyzing"
                                } else {
                                    "processing"
                                }
                                .into(),
                                completed_files: completed,
                                total_files: total,
                                current_path: current,
                                error_count: failures,
                            },
                        );
                    },
                )
            });
        if store.is_cancelled(&batch_id) {
            delete_batch_results(&database_path, &batch_id);
            let _ = store.finish_cancelled(&batch_id);
            if let Some(snapshot) = store.get(&batch_id) {
                emit_analysis_progress(&worker_app, &snapshot, "cancelled");
            }
            return;
        }
        match result {
            Ok(outcome) => {
                let result_ids = outcome
                    .records
                    .iter()
                    .map(|record| record.id.clone())
                    .collect();
                if store
                    .complete_with_failures(&batch_id, result_ids, outcome.failures)
                    .is_err()
                {
                    delete_batch_results(&database_path, &batch_id);
                    let _ = store.finish_cancelled(&batch_id);
                    if let Some(snapshot) = store.get(&batch_id) {
                        emit_analysis_progress(&worker_app, &snapshot, "cancelled");
                    }
                } else if let Some(snapshot) = store.get(&batch_id) {
                    emit_analysis_progress(&worker_app, &snapshot, "completed");
                }
            }
            Err(error) => {
                if store.is_cancelled(&batch_id) || store.fail(&batch_id, error).is_err() {
                    delete_batch_results(&database_path, &batch_id);
                    let _ = store.finish_cancelled(&batch_id);
                    if let Some(snapshot) = store.get(&batch_id) {
                        emit_analysis_progress(&worker_app, &snapshot, "cancelled");
                    }
                } else if let Some(snapshot) = store.get(&batch_id) {
                    emit_analysis_progress(&worker_app, &snapshot, "failed");
                }
            }
        }
    });
    Ok(response)
}

#[tauri::command]
pub fn get_analysis_batch<R: Runtime>(
    app: AppHandle<R>,
    batch_id: String,
) -> Result<AnalysisTaskSnapshot, AppError> {
    app.state::<AnalysisTaskStore>()
        .get(&batch_id)
        .ok_or_else(|| "分析批次不存在".to_string().into())
}

#[tauri::command]
pub fn cancel_analysis_batch<R: Runtime>(
    app: AppHandle<R>,
    batch_id: String,
) -> Result<(), AppError> {
    let store = app.state::<AnalysisTaskStore>();
    store.cancel(&batch_id)?;
    if let Some(snapshot) = store.get(&batch_id) {
        match snapshot.status {
            AnalysisBatchStatus::Cancelling => {
                emit_analysis_progress(&app, &snapshot, "cancelling");
            }
            AnalysisBatchStatus::Cancelled => {
                emit_analysis_progress(&app, &snapshot, "cancelled");
            }
            _ => {}
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_analysis_results<R: Runtime>(
    app: AppHandle<R>,
    batch_id: String,
) -> Result<Vec<AiAnalysisRecord>, AppError> {
    let connection = open_database(&app)?;
    let mut records = ai_repository::read_batch_results(&connection, &batch_id)
        .map_err(|error| error.to_string())?;
    for record in &mut records {
        if record.status == AnalysisResultStatus::Pending
            && fingerprint_file(Path::new(&record.source_path))
                .ok()
                .as_deref()
                != Some(record.content_fingerprint.as_str())
        {
            ai_repository::update_result_status(
                &connection,
                &record.id,
                AnalysisResultStatus::Expired,
            )
            .map_err(|error| error.to_string())?;
            record.status = AnalysisResultStatus::Expired;
        }
    }
    Ok(records)
}

#[tauri::command]
pub fn review_analysis_result<R: Runtime>(
    app: AppHandle<R>,
    request: ReviewAnalysisRequest,
) -> Result<Option<OperationDraft>, AppError> {
    suggestion_review::review_result(
        &open_database(&app)?,
        &request.result_id,
        request.action,
        request.suggested_filename,
        request.category_id,
    )
    .map_err(Into::into)
}

fn validate_categories(root: &Path, categories: Vec<Category>) -> Result<Vec<Category>, AppError> {
    if categories.is_empty() || categories.len() > 100 {
        return Err("分类数量必须位于 1 到 100 之间".to_string().into());
    }
    let mut ids = HashSet::new();
    let mut validated = Vec::with_capacity(categories.len());
    for mut category in categories {
        if path_policy::validate_category_id(&category.id).is_err()
            || !ids.insert(category.id.to_ascii_lowercase())
        {
            return Err("分类 ID 必须唯一，且只能包含字母、数字、连字符和下划线"
                .to_string()
                .into());
        }
        if category.name.trim().is_empty() {
            return Err("分类名称不能为空".to_string().into());
        }
        let directory =
            path_policy::category_directory_for_category(root, &category.id, &category.name)
                .map_err(AppError::from)?;
        if let Ok(metadata) = std::fs::symlink_metadata(&directory) {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err("分类目标路径已存在但不是普通目录".to_string().into());
            }
        }
        category.name = category.name.trim().into();
        category.description = category.description.trim().into();
        category.directory_path = directory.to_string_lossy().into();
        validated.push(category);
    }
    Ok(validated)
}

fn validate_template_id(id: &str) -> Result<(), AppError> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("模板 ID 不能为空，且只能包含字母、数字、连字符和下划线"
            .to_string()
            .into());
    }
    Ok(())
}

fn validate_template_categories(
    categories: Vec<TemplateCategory>,
) -> Result<Vec<TemplateCategory>, AppError> {
    if categories.is_empty() || categories.len() > 100 {
        return Err("模板分类数量必须位于 1 到 100 之间".to_string().into());
    }
    let mut ids = HashSet::new();
    let mut validated = Vec::with_capacity(categories.len());
    for mut category in categories {
        if category.id.is_empty()
            || !category.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            || !ids.insert(category.id.to_ascii_lowercase())
        {
            return Err("模板分类 ID 必须唯一，且只能包含字母、数字、连字符和下划线"
                .to_string()
                .into());
        }
        category.name = category.name.trim().into();
        category.description = category.description.trim().into();
        if category.name.is_empty() {
            return Err("模板分类名称不能为空".to_string().into());
        }
        validated.push(category);
    }
    Ok(validated)
}

fn validate_applied_categories(
    root: &Path,
    template: &CategoryTemplate,
    categories: Vec<Category>,
) -> Result<Vec<Category>, AppError> {
    if categories.len() != template.categories.len() {
        return Err("应用模板时分类数量不匹配".to_string().into());
    }
    let expected: std::collections::HashMap<_, _> = template
        .categories
        .iter()
        .map(|category| (category.id.as_str(), category))
        .collect();
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(categories.len());
    for mut category in categories {
        let Some(template_category) = expected.get(category.id.as_str()) else {
            return Err("应用模板时包含未知分类".to_string().into());
        };
        if !seen.insert(category.id.clone()) {
            return Err("应用模板时分类 ID 不能重复".to_string().into());
        }
        category.name = if template_category.name.trim().is_empty()
            || template_category.name.trim() == DEFAULT_CATEGORY_NAME
        {
            category.id.clone()
        } else {
            template_category.name.trim().into()
        };
        category.description = template_category.description.clone();
        category.directory_path = path_policy::category_directory(root, &category.id)
            .map_err(AppError::from)?
            .to_string_lossy()
            .into();
        validated.push(category);
    }
    if seen.len() != expected.len() {
        return Err("应用模板时缺少分类目录绑定".to_string().into());
    }
    Ok(validated)
}

fn require_current_root<R: Runtime>(
    app: &AppHandle<R>,
    root_path: &str,
) -> Result<PathBuf, AppError> {
    let root = path_policy::normalize_root(Path::new(root_path))?;
    if !app
        .state::<watcher::WatcherState>()
        .is_current_root(&root)?
    {
        return Err("只能分析当前授权根目录内的文件".to_string().into());
    }
    Ok(root)
}

fn open_database<R: Runtime>(app: &AppHandle<R>) -> Result<rusqlite::Connection, AppError> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    database::open_database(&app_paths::database_path(&directory))
        .map_err(|error| error.to_string().into())
}

fn configured_model(model: Option<String>) -> String {
    model
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.into())
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

const DEFAULT_CATEGORY_NAME: &str = "新分类";
