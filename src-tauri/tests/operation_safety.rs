use ai_file_organizer_lib::models::operation::{
    OperationDraft, OperationDraftItem, OperationValidationStatus,
};
use ai_file_organizer_lib::services::operation_executor::execute_plan;
use ai_file_organizer_lib::services::operation_validator::validate_draft;
use ai_file_organizer_lib::services::plan_store::ValidatedPlan;
use ai_file_organizer_lib::storage::database::open_memory_database;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ai-file-organizer-phase4-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::canonicalize(root).unwrap()
}

#[test]
fn validator_accepts_an_existing_same_root_move_without_touching_disk() {
    let root = temp_root("valid-move");
    let source = root.join("source.txt");
    let destination = root.join("archive");
    fs::write(&source, "content").unwrap();
    fs::create_dir(&destination).unwrap();

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: source.to_string_lossy().into(),
            destination_directory: destination.to_string_lossy().into(),
        }],
    })
    .unwrap();

    assert!(preview.can_confirm);
    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].status, OperationValidationStatus::Valid);
    assert_eq!(preview.items[0].target_path, destination.join("source.txt"));
    assert!(source.exists());
    assert!(!destination.join("source.txt").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn validator_rejects_directory_target_conflict_and_root_escape() {
    let root = temp_root("invalid-draft");
    let source = root.join("source.txt");
    let destination = root.join("archive");
    let conflict = destination.join("source.txt");
    let outside = root.parent().unwrap().join("outside.txt");
    fs::write(&source, "content").unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(&conflict, "existing").unwrap();
    fs::write(&outside, "outside").unwrap();

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![
            OperationDraftItem::Move {
                source_path: source.to_string_lossy().into(),
                destination_directory: destination.to_string_lossy().into(),
            },
            OperationDraftItem::Rename {
                source_path: outside.to_string_lossy().into(),
                new_name: "renamed.txt".into(),
            },
        ],
    })
    .unwrap();

    assert!(!preview.can_confirm);
    assert_eq!(preview.items[0].status, OperationValidationStatus::Invalid);
    assert_eq!(preview.items[1].status, OperationValidationStatus::Invalid);
    assert!(preview.items.iter().all(|item| item.reason.is_some()));
    assert!(source.exists());
    assert!(conflict.exists());

    let _ = fs::remove_file(outside);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn validator_rejects_directories_and_invalid_names() {
    let root = temp_root("invalid-source");
    let source_dir = root.join("folder");
    fs::create_dir(&source_dir).unwrap();

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Rename {
            source_path: source_dir.to_string_lossy().into(),
            new_name: "bad/name.txt".into(),
        }],
    })
    .unwrap();

    assert!(!preview.can_confirm);
    assert_eq!(preview.items[0].status, OperationValidationStatus::Invalid);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn validator_rejects_a_file_symlink_even_when_its_target_stays_inside_root() {
    let root = temp_root("symlink-source");
    let actual = root.join("actual.txt");
    let link = root.join("linked.txt");
    let destination = root.join("archive");
    fs::write(&actual, "content").unwrap();
    fs::create_dir(&destination).unwrap();
    let link_result = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&actual, &link)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(&actual, &link)
        }
    };
    if link_result.is_err() {
        let _ = fs::remove_dir_all(root);
        return;
    }

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: link.to_string_lossy().into(),
            destination_directory: destination.to_string_lossy().into(),
        }],
    })
    .unwrap();

    assert!(!preview.can_confirm);
    assert_eq!(preview.items[0].status, OperationValidationStatus::Invalid);
    assert!(link.exists());
    assert!(actual.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn validator_rejects_a_source_path_through_an_inside_root_directory_symlink() {
    let root = temp_root("symlink-directory-component");
    let actual_directory = root.join("actual");
    let linked_directory = root.join("linked");
    let source = linked_directory.join("source.txt");
    let destination = root.join("archive");
    fs::create_dir(&actual_directory).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(actual_directory.join("source.txt"), "content").unwrap();
    let link_result = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&actual_directory, &linked_directory)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(&actual_directory, &linked_directory)
        }
    };
    if link_result.is_err() {
        let _ = fs::remove_dir_all(root);
        return;
    }

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::Move {
            source_path: source.to_string_lossy().into(),
            destination_directory: destination.to_string_lossy().into(),
        }],
    })
    .unwrap();

    assert!(!preview.can_confirm);
    assert_eq!(preview.items[0].status, OperationValidationStatus::Invalid);
    assert!(source.exists());
    assert!(actual_directory.join("source.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ai_organize_draft_uses_the_suggested_name_and_binds_the_content_fingerprint() {
    let root = temp_root("ai-organize");
    let source = root.join("notes.md");
    let destination = root.join("work");
    fs::write(&source, "项目会议纪要").unwrap();
    fs::create_dir(&destination).unwrap();
    let fingerprint = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::AiOrganize {
            source_path: source.to_string_lossy().into(),
            category_id: "work".into(),
            new_name: "项目会议.md".into(),
            content_fingerprint: fingerprint.clone(),
        }],
    })
    .unwrap();

    assert!(preview.can_confirm);
    assert_eq!(
        preview.items[0].target_path,
        destination.join("项目会议.md")
    );
    assert_eq!(
        preview.items[0].content_fingerprint.as_deref(),
        Some(fingerprint.as_str())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ai_organize_preview_allows_a_missing_category_directory_without_creating_it() {
    let root = temp_root("ai-organize-missing-directory");
    let source = root.join("notes.md");
    let destination = root.join("study");
    fs::write(&source, "学习资料").unwrap();
    let fingerprint = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::AiOrganize {
            source_path: source.to_string_lossy().into(),
            category_id: "study".into(),
            new_name: "学习资料.md".into(),
            content_fingerprint: fingerprint,
        }],
    })
    .unwrap();

    assert!(preview.can_confirm);
    assert!(!destination.exists());
    assert!(preview.items[0].will_create_directory);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ai_organize_execution_creates_the_missing_category_directory_and_moves_once() {
    let root = temp_root("ai-organize-create-on-execute");
    let source = root.join("notes.md");
    let destination = root.join("game");
    fs::write(&source, "游戏资料").unwrap();
    let fingerprint = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::AiOrganize {
            source_path: source.to_string_lossy().into(),
            category_id: "game".into(),
            new_name: "游戏资料.md".into(),
            content_fingerprint: fingerprint,
        }],
    })
    .unwrap();
    assert!(preview.can_confirm);
    assert!(!destination.exists());

    let connection = open_memory_database().unwrap();
    let result = execute_plan(
        &connection,
        &ValidatedPlan {
            plan_id: "plan-ai-create".into(),
            items: preview.items,
        },
    )
    .unwrap();

    assert_eq!(
        result.items[0].status,
        ai_file_organizer_lib::models::operation::OperationResultStatus::Succeeded
    );
    assert!(destination.join("游戏资料.md").exists());
    assert!(!source.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ai_draft_with_a_stale_content_fingerprint_is_rejected_during_preview() {
    let root = temp_root("ai-stale");
    let source = root.join("notes.md");
    fs::write(&source, "当前正文").unwrap();

    let preview = validate_draft(&OperationDraft {
        root_path: root.to_string_lossy().into(),
        items: vec![OperationDraftItem::AiRename {
            source_path: source.to_string_lossy().into(),
            new_name: "新名称.md".into(),
            content_fingerprint: "stale".into(),
        }],
    })
    .unwrap();

    assert!(!preview.can_confirm);
    assert!(
        preview.items[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("内容指纹")
    );
    let _ = fs::remove_dir_all(root);
}
