use ai_file_organizer_lib::models::operation::{
    FileSnapshot, OperationPreview, OperationPreviewItem, OperationType, OperationValidationStatus,
};
use ai_file_organizer_lib::services::plan_store::{PlanState, PlanStore};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn preview() -> OperationPreview {
    OperationPreview {
        can_confirm: true,
        items: vec![OperationPreviewItem {
            index: 0,
            operation: OperationType::Rename,
            source_path: PathBuf::from("/root/source.txt"),
            target_path: PathBuf::from("/root/renamed.txt"),
            status: OperationValidationStatus::Valid,
            reason: None,
            snapshot: Some(FileSnapshot {
                kind: "file".into(),
                size: 1,
                modified_ms: Some(1),
                file_identity: Some("test:file".into()),
                volume_id: Some("test:volume".into()),
            }),
            content_fingerprint: None,
            will_create_directory: false,
        }],
    }
}

#[test]
fn plan_store_expires_cancels_and_consumes_once() {
    let store = PlanStore::default();
    let created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let plan = store.create_at(preview(), created_at).unwrap();

    assert_eq!(plan.expires_at, created_at + Duration::from_secs(600));
    assert_eq!(store.state(&plan.plan_id, created_at), PlanState::Valid);
    assert_eq!(
        store.state(&plan.plan_id, created_at + Duration::from_secs(601)),
        PlanState::Expired
    );
    assert!(
        store
            .consume(&plan.plan_id, created_at + Duration::from_secs(601))
            .is_err()
    );

    let second = store.create_at(preview(), created_at).unwrap();
    assert!(store.cancel(&second.plan_id).is_ok());
    assert_eq!(
        store.state(&second.plan_id, created_at),
        PlanState::Canceled
    );
    assert!(store.consume(&second.plan_id, created_at).is_err());

    let third = store.create_at(preview(), created_at).unwrap();
    assert!(store.consume(&third.plan_id, created_at).is_ok());
    assert_eq!(store.state(&third.plan_id, created_at), PlanState::Consumed);
    assert!(store.consume(&third.plan_id, created_at).is_err());
}
