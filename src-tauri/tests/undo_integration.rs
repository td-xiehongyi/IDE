use ai_file_organizer_lib::models::operation::{
    OperationDraft, OperationDraftItem, OperationResultStatus,
};
use ai_file_organizer_lib::services::operation_executor::execute_plan;
use ai_file_organizer_lib::services::operation_validator::validate_draft;
use ai_file_organizer_lib::services::plan_store::ValidatedPlan;
use ai_file_organizer_lib::services::undo_service::undo_history;
use ai_file_organizer_lib::storage::database::open_memory_database;
use std::fs;
use std::path::PathBuf;

fn temp_root() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ai-file-organizer-phase4-undo-{}-{nonce}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

#[test]
fn undo_restores_a_successful_move_and_cannot_repeat_it() {
    let root = temp_root();
    let source = root.join("source.txt");
    let archive = root.join("archive");
    fs::write(&source, "content").unwrap();
    fs::create_dir(&archive).unwrap();

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: source.to_string_lossy().into(),
            destination_directory: archive.to_string_lossy().into(),
        }],
    })
    .unwrap();
    let database = open_memory_database().unwrap();
    let execution = execute_plan(
        &database,
        &ValidatedPlan {
            plan_id: "plan-undo".into(),
            items: preview.items,
        },
    )
    .unwrap();
    assert_eq!(execution.items[0].status, OperationResultStatus::Succeeded);
    let history_id = execution.items[0].history_id.unwrap();
    assert!(!source.exists());
    assert!(archive.join("source.txt").exists());

    let undone = undo_history(&database, history_id).unwrap();
    assert_eq!(undone.status, OperationResultStatus::Succeeded);
    assert!(source.exists());
    assert!(!archive.join("source.txt").exists());
    assert!(undo_history(&database, history_id).is_err());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn undo_rejects_a_new_file_at_the_original_path_without_overwriting_it() {
    let root = temp_root();
    let source = root.join("source.txt");
    let archive = root.join("archive");
    fs::write(&source, "content").unwrap();
    fs::create_dir(&archive).unwrap();
    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: source.to_string_lossy().into(),
            destination_directory: archive.to_string_lossy().into(),
        }],
    })
    .unwrap();
    let database = open_memory_database().unwrap();
    let execution = execute_plan(
        &database,
        &ValidatedPlan {
            plan_id: "plan-undo-conflict".into(),
            items: preview.items,
        },
    )
    .unwrap();
    fs::write(&source, "new file").unwrap();
    let error = undo_history(&database, execution.items[0].history_id.unwrap()).unwrap_err();
    assert!(error.contains("被占用"));
    assert_eq!(fs::read_to_string(&source).unwrap(), "new file");
    assert!(archive.join("source.txt").exists());
    let _ = fs::remove_dir_all(root);
}
