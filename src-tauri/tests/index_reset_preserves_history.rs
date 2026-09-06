use ai_file_organizer_lib::models::operation::{
    FileSnapshot, HistoryAction, OperationHistoryItem, OperationResultStatus, OperationType,
};
use ai_file_organizer_lib::storage::database::open_memory_database;
use ai_file_organizer_lib::storage::file_repository::reset_file_index;
use ai_file_organizer_lib::storage::operation_repository::{
    count_history, insert_history, read_history,
};
use std::path::PathBuf;

#[test]
fn index_reset_keeps_persisted_operation_history() {
    let connection = open_memory_database().unwrap();
    let record = OperationHistoryItem {
        id: 0,
        batch_id: "batch-1".into(),
        action: HistoryAction::Execute,
        operation: OperationType::Move,
        source_path: PathBuf::from("/root/source.txt"),
        target_path: PathBuf::from("/root/archive/source.txt"),
        status: OperationResultStatus::Succeeded,
        reason: None,
        created_at: "100".into(),
        undo_status: ai_file_organizer_lib::models::operation::UndoStatus::Available,
        undo_reason: None,
    };
    let snapshot = FileSnapshot {
        kind: "file".into(),
        size: 1,
        modified_ms: Some(1),
        file_identity: Some("file".into()),
        volume_id: Some("volume".into()),
    };

    let id = insert_history(&connection, &record, Some(&snapshot), None).unwrap();
    assert_eq!(count_history(&connection).unwrap(), 1);

    reset_file_index(&connection).unwrap();

    let history = read_history(&connection, 50, 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, id);
    assert_eq!(history[0].batch_id, "batch-1");
}
