use ai_file_organizer_lib::models::operation::{
    OperationDraft, OperationDraftItem, OperationResultStatus,
};
use ai_file_organizer_lib::services::operation_executor::execute_plan;
use ai_file_organizer_lib::services::operation_validator::validate_draft;
use ai_file_organizer_lib::services::plan_store::ValidatedPlan;
use ai_file_organizer_lib::storage::database::open_memory_database;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn temp_root() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ai-file-organizer-phase4-batch-{}-{nonce}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

#[test]
fn runtime_failure_stops_later_items_and_persists_real_statuses() {
    let root = temp_root();
    let archive = root.join("archive");
    fs::create_dir(&archive).unwrap();
    for name in ["one.txt", "two.txt", "three.txt"] {
        fs::write(root.join(name), name).unwrap();
    }

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: ["one.txt", "two.txt", "three.txt"]
            .into_iter()
            .map(|name| OperationDraftItem::Move {
                source_path: root.join(name).to_string_lossy().into(),
                destination_directory: archive.to_string_lossy().into(),
            })
            .collect(),
    })
    .unwrap();
    assert!(preview.can_confirm);

    fs::write(archive.join("two.txt"), "appeared after preview").unwrap();
    let database = open_memory_database().unwrap();
    let result = execute_plan(
        &database,
        &ValidatedPlan {
            plan_id: "plan-test".into(),
            items: preview.items,
        },
    )
    .unwrap();

    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.status)
            .collect::<Vec<_>>(),
        vec![
            OperationResultStatus::Succeeded,
            OperationResultStatus::Failed,
            OperationResultStatus::NotExecuted
        ]
    );
    assert!(archive.join("one.txt").exists());
    assert!(root.join("two.txt").exists());
    assert!(archive.join("two.txt").exists());
    assert!(root.join("three.txt").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn execution_rejects_source_metadata_changes_after_preview() {
    let root = temp_root();
    let archive = root.join("archive");
    let source = root.join("source.txt");
    fs::create_dir(&archive).unwrap();
    fs::write(&source, "before").unwrap();
    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: source.to_string_lossy().into(),
            destination_directory: archive.to_string_lossy().into(),
        }],
    })
    .unwrap();
    fs::write(&source, "changed after preview").unwrap();
    let database = open_memory_database().unwrap();
    let result = execute_plan(
        &database,
        &ValidatedPlan {
            plan_id: "plan-source-change".into(),
            items: preview.items,
        },
    )
    .unwrap();
    assert_eq!(result.items[0].status, OperationResultStatus::Failed);
    assert!(source.exists());
    assert!(!archive.join("source.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn execution_rechecks_ai_content_fingerprint_after_preview() {
    let root = temp_root();
    let source = root.join("source.md");
    fs::write(&source, "AAAA").unwrap();
    let fingerprint = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::AiRename {
            source_path: source.to_string_lossy().into(),
            new_name: "renamed.md".into(),
            content_fingerprint: fingerprint,
        }],
    })
    .unwrap();
    fs::write(&source, "BBBB").unwrap();

    let database = open_memory_database().unwrap();
    let result = execute_plan(
        &database,
        &ValidatedPlan {
            plan_id: "plan-ai-change".into(),
            items: preview.items,
        },
    )
    .unwrap();

    assert_eq!(result.items[0].status, OperationResultStatus::Failed);
    assert!(source.exists());
    assert!(!root.join("renamed.md").exists());
    let _ = fs::remove_dir_all(root);
}
