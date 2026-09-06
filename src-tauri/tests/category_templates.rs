use std::fs;

use ai_file_organizer_lib::models::ai::{AnalysisResultStatus, Category, TemplateCategory};
use ai_file_organizer_lib::storage::{ai_repository, database};

fn temp_root(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ai-file-organizer-template-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    fs::canonicalize(path).unwrap()
}

#[test]
fn template_round_trip_increments_version_without_directory_paths() {
    let mut connection = database::open_memory_database().unwrap();
    let categories = vec![TemplateCategory {
        id: "work".into(),
        name: "工作".into(),
        description: "工作资料".into(),
        default_enabled: true,
    }];

    let first = ai_repository::upsert_category_template(
        &mut connection,
        "default",
        "默认模板",
        &categories,
        "1",
    )
    .unwrap();
    assert_eq!(first.version, 1);
    assert_eq!(first.categories, categories);

    let second = ai_repository::upsert_category_template(
        &mut connection,
        "default",
        "默认模板（更新）",
        &categories,
        "2",
    )
    .unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(
        ai_repository::read_category_templates(&connection).unwrap(),
        vec![second]
    );
}

#[test]
fn applying_template_copies_root_categories_and_keeps_template_binding() {
    let root = temp_root("apply");
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let template = ai_repository::upsert_category_template(
        &mut connection,
        "default",
        "默认模板",
        &[TemplateCategory {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            default_enabled: true,
        }],
        "1",
    )
    .unwrap();
    let categories = vec![Category {
        id: "work".into(),
        name: "工作".into(),
        description: "工作资料".into(),
        directory_path: work.to_string_lossy().into(),
        enabled: true,
    }];

    ai_repository::replace_categories(&mut connection, &root.to_string_lossy(), &categories)
        .unwrap();
    ai_repository::bind_root_to_category_template(
        &connection,
        &root.to_string_lossy(),
        &template.id,
        template.version,
    )
    .unwrap();

    assert_eq!(
        ai_repository::read_categories(&connection, &root.to_string_lossy()).unwrap(),
        categories
    );
    assert_eq!(
        ai_repository::read_root_category_template(&connection, &root.to_string_lossy()).unwrap(),
        Some(("default".into(), 1))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_template_keeps_applied_root_categories() {
    let root = temp_root("delete-template");
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let mut connection = database::open_memory_database().unwrap();
    let template = ai_repository::upsert_category_template(
        &mut connection,
        "default",
        "默认模板",
        &[TemplateCategory {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            default_enabled: true,
        }],
        "1",
    )
    .unwrap();
    ai_repository::replace_categories(
        &mut connection,
        &root.to_string_lossy(),
        &[Category {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            directory_path: work.to_string_lossy().into(),
            enabled: true,
        }],
    )
    .unwrap();
    ai_repository::bind_root_to_category_template(
        &connection,
        &root.to_string_lossy(),
        &template.id,
        template.version,
    )
    .unwrap();

    assert!(ai_repository::delete_category_template(&connection, &template.id).unwrap());
    assert!(
        ai_repository::read_category_template(&connection, &template.id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ai_repository::read_categories(&connection, &root.to_string_lossy())
            .unwrap()
            .len(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_category_removes_only_configuration_and_pending_reference_blocks_it() {
    let root = temp_root("delete");
    let work = root.join("work");
    let source = root.join("notes.md");
    fs::create_dir_all(&work).unwrap();
    fs::write(&source, "正文").unwrap();
    let mut connection = database::open_memory_database().unwrap();
    ai_repository::replace_categories(
        &mut connection,
        &root.to_string_lossy(),
        &[Category {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            directory_path: work.to_string_lossy().into(),
            enabled: true,
        }],
    )
    .unwrap();

    assert_eq!(
        ai_repository::count_pending_category_references(
            &connection,
            &root.to_string_lossy(),
            "work",
        )
        .unwrap(),
        0
    );
    ai_repository::delete_category(&connection, &root.to_string_lossy(), "work").unwrap();
    assert!(
        ai_repository::read_categories(&connection, &root.to_string_lossy())
            .unwrap()
            .is_empty()
    );
    assert!(work.is_dir());
    assert!(source.is_file());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pending_result_reference_is_counted_for_safe_delete() {
    let connection = database::open_memory_database().unwrap();
    connection
        .execute(
            "INSERT INTO ai_analysis_results (
                id, batch_id, root_path, source_path, content_fingerprint, provider, model,
                prompt_version, summary, keywords_json, suggested_filename, category_id,
                confidence, reason, status, created_at
             ) VALUES ('r1', 'b1', 'C:/root', 'C:/root/a.md', 'fp', 'ollama', 'qwen2.5:7b',
                       'phase5-v1', '摘要', '[\"资料\"]', 'a.md', 'work', 0.9, '原因', 'pending', '1')",
            [],
        )
        .unwrap();
    assert_eq!(
        ai_repository::count_pending_category_references(&connection, "C:/root", "work").unwrap(),
        1
    );
    assert_eq!(AnalysisResultStatus::Pending.as_str(), "pending");
}
