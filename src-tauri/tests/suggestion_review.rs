use std::fs;

use ai_file_organizer_lib::models::ai::{AiAnalysisRecord, AnalysisResultStatus, Category};
use ai_file_organizer_lib::models::operation::OperationDraftItem;
use ai_file_organizer_lib::services::content_extractor::fingerprint_file;
use ai_file_organizer_lib::services::suggestion_review::{ReviewAction, review_result};
use ai_file_organizer_lib::storage::{ai_repository, database};

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ai-file-organizer-review-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

fn seed(
    connection: &mut rusqlite::Connection,
    root: &std::path::Path,
    source: &std::path::Path,
) -> AiAnalysisRecord {
    let category_directory = root.join("work");
    fs::create_dir_all(&category_directory).unwrap();
    ai_repository::replace_categories(
        connection,
        &root.to_string_lossy(),
        &[Category {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            directory_path: category_directory.to_string_lossy().into(),
            enabled: true,
        }],
    )
    .unwrap();
    let record = AiAnalysisRecord {
        id: "result-1".into(),
        batch_id: "batch-1".into(),
        root_path: root.to_string_lossy().into(),
        source_path: source.to_string_lossy().into(),
        content_fingerprint: fingerprint_file(source).unwrap(),
        provider: "ollama".into(),
        model: "qwen2.5:7b".into(),
        prompt_version: "phase5-v1".into(),
        template_id: None,
        template_version: None,
        summary: "会议纪要".into(),
        keywords: vec!["会议".into()],
        suggested_filename: "项目会议.md".into(),
        category_id: Some("work".into()),
        confidence: 0.9,
        reason: "工作资料".into(),
        status: AnalysisResultStatus::Pending,
        created_at: "1".into(),
    };
    ai_repository::insert_analysis_result(connection, &record).unwrap();
    record
}

#[test]
fn accepting_a_current_result_returns_a_fingerprint_bound_operation_draft() {
    let root = temp_root("accept");
    let source = root.join("notes.md");
    fs::write(&source, "正文").unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let record = seed(&mut connection, &root, &source);

    let draft = review_result(
        &connection,
        &record.id,
        ReviewAction::Accept,
        Some("最终名称.md".into()),
        Some("work".into()),
    )
    .unwrap()
    .unwrap();

    assert_eq!(draft.root_path, root.to_string_lossy());
    assert!(matches!(
        &draft.items[0],
        OperationDraftItem::AiOrganize { new_name, content_fingerprint, .. }
            if new_name == "最终名称.md" && content_fingerprint.as_str() == record.content_fingerprint
    ));
    assert_eq!(
        ai_repository::read_result(&connection, &record.id)
            .unwrap()
            .unwrap()
            .status,
        AnalysisResultStatus::Accepted
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn accepting_a_generated_category_with_a_safe_name_targets_the_name_directory() {
    let root = temp_root("generated-category-name");
    let source = root.join("notes.md");
    fs::write(&source, "正文").unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let record = AiAnalysisRecord {
        id: "result-generated".into(),
        batch_id: "batch-generated".into(),
        root_path: root.to_string_lossy().into(),
        source_path: source.to_string_lossy().into(),
        content_fingerprint: fingerprint_file(&source).unwrap(),
        provider: "ollama".into(),
        model: "qwen2.5:7b".into(),
        prompt_version: "phase5-v1".into(),
        template_id: None,
        template_version: None,
        summary: "学习资料".into(),
        keywords: vec!["学习".into()],
        suggested_filename: "学习资料.md".into(),
        category_id: Some("category_2".into()),
        confidence: 0.9,
        reason: "学习资料".into(),
        status: AnalysisResultStatus::Pending,
        created_at: "1".into(),
    };
    ai_repository::replace_categories(
        &mut connection,
        &root.to_string_lossy(),
        &[Category {
            id: "category_2".into(),
            name: "study".into(),
            description: "学习资料".into(),
            directory_path: root.join("category_2").to_string_lossy().into(),
            enabled: true,
        }],
    )
    .unwrap();
    ai_repository::insert_analysis_result(&connection, &record).unwrap();

    let draft = review_result(
        &connection,
        &record.id,
        ReviewAction::Accept,
        None,
        Some("category_2".into()),
    )
    .unwrap()
    .unwrap();

    assert!(matches!(
        &draft.items[0],
        OperationDraftItem::AiOrganize { category_id, .. } if category_id == "study"
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejecting_a_result_updates_status_without_creating_an_operation() {
    let root = temp_root("reject");
    let source = root.join("notes.md");
    fs::write(&source, "正文").unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let record = seed(&mut connection, &root, &source);

    assert!(
        review_result(&connection, &record.id, ReviewAction::Reject, None, None)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ai_repository::read_result(&connection, &record.id)
            .unwrap()
            .unwrap()
            .status,
        AnalysisResultStatus::Rejected
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn accepting_a_stale_result_marks_it_expired_and_returns_no_draft() {
    let root = temp_root("stale");
    let source = root.join("notes.md");
    fs::write(&source, "原文").unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let record = seed(&mut connection, &root, &source);
    fs::write(&source, "替换").unwrap();

    assert!(
        review_result(&connection, &record.id, ReviewAction::Accept, None, None)
            .unwrap_err()
            .contains("过期")
    );
    assert_eq!(
        ai_repository::read_result(&connection, &record.id)
            .unwrap()
            .unwrap()
            .status,
        AnalysisResultStatus::Expired
    );
    fs::remove_dir_all(root).unwrap();
}
