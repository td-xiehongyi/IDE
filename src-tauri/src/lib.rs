pub mod ai;
pub mod commands;
pub mod models;
pub mod services;
pub mod storage;

pub fn app_builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(services::watcher::WatcherState::default())
        .manage(services::plan_store::PlanStore::default())
        .manage(services::analysis_task_store::AnalysisTaskStore::default())
        .invoke_handler(tauri::generate_handler![
            commands::scan_directory,
            commands::get_index_status,
            commands::restore_recent_index,
            commands::rebuild_index,
            commands::search::search_files,
            commands::search::list_explorer_children,
            commands::search::resolve_indexed_file,
            commands::operations::preview_operations,
            commands::operations::cancel_operation_plan,
            commands::operations::execute_operation_plan,
            commands::operations::get_operation_history,
            commands::operations::undo_operation,
            commands::ai::get_ai_provider_status,
            commands::ai::save_ai_categories,
            commands::ai::get_ai_categories,
            commands::ai::get_ai_category_templates,
            commands::ai::save_ai_category_template,
            commands::ai::delete_ai_category_template,
            commands::ai::apply_ai_category_template,
            commands::ai::delete_ai_category,
            commands::ai::start_analysis_batch,
            commands::ai::get_analysis_batch,
            commands::ai::cancel_analysis_batch,
            commands::ai::get_analysis_results,
            commands::ai::review_analysis_result,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_builder(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running AI File Organizer");
}
