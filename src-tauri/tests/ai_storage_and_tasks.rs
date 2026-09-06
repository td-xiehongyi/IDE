use std::fs;
use std::time::SystemTime;

use ai_file_organizer_lib::models::ai::{
    AiAnalysisRecord, AnalysisBatchStatus, AnalysisFailure, AnalysisResultStatus, Category,
};
use ai_file_organizer_lib::services::analysis_task_store::AnalysisTaskStore;
use ai_file_organizer_lib::storage::{ai_repository, database, file_repository};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ai-file-organizer-ai-storage-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn persists_categories_and_derived_results_without_raw_text() {
    let mut connection = database::open_memory_database().unwrap();
    let category = Category {
        id: "work".into(),
        name: "工作".into(),
        description: "工作资料".into(),
        directory_path: r"C:\root\work".into(),
        enabled: true,
    };
    ai_repository::replace_categories(&mut connection, r"C:\root", &[category.clone()]).unwrap();
    assert_eq!(
        ai_repository::read_categories(&connection, r"C:\root").unwrap(),
        vec![category]
    );

    let record = AiAnalysisRecord {
        id: "result-1".into(),
        batch_id: "analysis-1".into(),
        root_path: r"C:\root".into(),
        source_path: r"C:\root\notes.md".into(),
        content_fingerprint: "abc123".into(),
        provider: "ollama".into(),
        model: "qwen2.5:7b".into(),
        prompt_version: "phase5-v1".into(),
        template_id: None,
        template_version: None,
        summary: "会议纪要".into(),
        keywords: vec!["项目".into()],
        suggested_filename: "项目会议.md".into(),
        category_id: Some("work".into()),
        confidence: 0.9,
        reason: "工作资料".into(),
        status: AnalysisResultStatus::Pending,
        created_at: "123".into(),
    };
    ai_repository::insert_analysis_result(&connection, &record).unwrap();
    assert_eq!(
        ai_repository::read_batch_results(&connection, "analysis-1").unwrap(),
        vec![record]
    );
    ai_repository::delete_batch_results(&connection, "analysis-1").unwrap();
    assert!(
        ai_repository::read_batch_results(&connection, "analysis-1")
            .unwrap()
            .is_empty()
    );

    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(ai_analysis_results)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        !columns
            .iter()
            .any(|column| matches!(column.as_str(), "text" | "content" | "raw_text"))
    );
}

#[test]
fn rebuilding_the_file_index_preserves_ai_configuration_and_results() {
    let mut connection = database::open_memory_database().unwrap();
    ai_repository::replace_categories(
        &mut connection,
        r"C:\root",
        &[Category {
            id: "work".into(),
            name: "工作".into(),
            description: String::new(),
            directory_path: r"C:\root\work".into(),
            enabled: true,
        }],
    )
    .unwrap();
    file_repository::reset_file_index(&connection).unwrap();
    assert_eq!(
        ai_repository::read_categories(&connection, r"C:\root")
            .unwrap()
            .len(),
        1
    );
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 5);
}

#[test]
fn category_directory_migration_rewrites_only_configuration_paths() {
    let root = temp_dir("category-migration");
    let old_directory = root.join("legacy");
    fs::create_dir(&old_directory).unwrap();
    fs::write(old_directory.join("keep.txt"), "keep").unwrap();
    let root_string = root.to_string_lossy().to_string();
    let mut connection = database::open_memory_database().unwrap();
    ai_repository::replace_categories(
        &mut connection,
        &root_string,
        &[Category {
            id: "game".into(),
            name: "游戏".into(),
            description: String::new(),
            directory_path: old_directory.to_string_lossy().into(),
            enabled: true,
        }],
    )
    .unwrap();

    ai_repository::migrate_category_directories(&connection).unwrap();
    let categories = ai_repository::read_categories(&connection, &root_string).unwrap();
    assert_eq!(
        categories[0].directory_path,
        root.join("game").to_string_lossy()
    );
    assert!(old_directory.join("keep.txt").exists());
    assert!(!root.join("game").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn task_store_allows_one_active_batch_and_supports_progress_and_cancel() {
    let store = AnalysisTaskStore::default();
    let first = store
        .create(vec!["a.md".into(), "b.md".into()], SystemTime::now())
        .unwrap();
    assert!(
        store
            .create(vec!["c.md".into()], SystemTime::now())
            .is_err()
    );

    store.mark_running(&first).unwrap();
    store
        .update_progress(&first, 1, Some("b.md".into()))
        .unwrap();
    let snapshot = store.get(&first).unwrap();
    assert_eq!(snapshot.status, AnalysisBatchStatus::Running);
    assert_eq!(snapshot.completed_files, 1);
    assert_eq!(snapshot.current_path.as_deref(), Some("b.md"));

    store.cancel(&first).unwrap();
    assert!(store.is_cancelled(&first));
    assert_eq!(
        store.get(&first).unwrap().status,
        AnalysisBatchStatus::Cancelling
    );
    assert_eq!(store.get(&first).unwrap().completed_files, 1);
    assert_eq!(
        store.get(&first).unwrap().current_path.as_deref(),
        Some("b.md")
    );
    assert!(
        store
            .create(vec!["c.md".into()], SystemTime::now())
            .is_err()
    );

    store.finish_cancelled(&first).unwrap();
    assert_eq!(
        store.get(&first).unwrap().status,
        AnalysisBatchStatus::Cancelled
    );
    let second = store
        .create(vec!["c.md".into()], SystemTime::now())
        .unwrap();
    assert_ne!(first, second);
}

#[test]
fn cancelling_task_cannot_be_completed_or_failed() {
    let store = AnalysisTaskStore::default();
    let batch = store
        .create(vec!["a.md".into()], SystemTime::now())
        .unwrap();
    store.mark_running(&batch).unwrap();
    store.cancel(&batch).unwrap();

    assert!(store.complete(&batch, vec!["result-1".into()]).is_err());
    assert!(store.fail(&batch, "provider failed".into()).is_err());
    assert_eq!(
        store.get(&batch).unwrap().status,
        AnalysisBatchStatus::Cancelling
    );
}

#[test]
fn completed_task_exposes_result_ids_and_releases_the_single_task_slot() {
    let root = temp_dir("task");
    let store = AnalysisTaskStore::default();
    let batch = store
        .create(
            vec![root.join("a.md").to_string_lossy().into()],
            SystemTime::now(),
        )
        .unwrap();
    store.mark_running(&batch).unwrap();
    store
        .complete_with_failures(
            &batch,
            vec!["result-1".into()],
            vec![AnalysisFailure {
                source_path: "bad.pdf".into(),
                reason: "正文为空".into(),
            }],
        )
        .unwrap();
    let snapshot = store.get(&batch).unwrap();
    assert_eq!(snapshot.status, AnalysisBatchStatus::Completed);
    assert_eq!(snapshot.result_ids, vec!["result-1"]);
    assert_eq!(snapshot.failures[0].source_path, "bad.pdf");
    assert!(
        store
            .create(vec!["next.md".into()], SystemTime::now())
            .is_ok()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn queued_task_cancelled_before_worker_start_cannot_return_to_running() {
    let store = AnalysisTaskStore::default();
    let batch = store
        .create(vec!["a.md".into()], SystemTime::now())
        .unwrap();
    store.cancel(&batch).unwrap();

    assert!(store.mark_running(&batch).is_err());
    assert_eq!(
        store.get(&batch).unwrap().status,
        AnalysisBatchStatus::Cancelled
    );
}
