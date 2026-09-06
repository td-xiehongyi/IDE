use ai_file_organizer_lib::models::file_entry::EntryKind;
use ai_file_organizer_lib::models::search::{SearchQuery, SearchSortDirection, SearchSortField};
use ai_file_organizer_lib::services::{scanner, watcher};
use ai_file_organizer_lib::storage::{database, file_repository};
use notify::{
    Event, EventKind,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use std::fs;
use std::path::PathBuf;

#[test]
fn watcher_event_indexes_new_file_without_following_outside_paths() {
    let root =
        std::env::temp_dir().join(format!("ai-file-organizer-watcher-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let file = root.join("new.txt");
    fs::write(&file, "hello").unwrap();
    let database_path = root.join("index.sqlite");
    let mut connection = database::open_database(&database_path).unwrap();
    let output = scanner::scan_directory(&root).unwrap();
    let root_id = file_repository::upsert_scan_root(
        &connection,
        &file_repository::ScanRoot {
            path: root.to_string_lossy().into(),
            normalized_path: root.to_string_lossy().into(),
            created_at: "now".into(),
            last_scan_at: None,
        },
    )
    .unwrap();
    file_repository::replace_entries_for_root(&mut connection, root_id, &[], &[], "now").unwrap();
    fs::remove_file(&file).unwrap();
    fs::write(&file, "hello again").unwrap();
    let mut event = Event::new(EventKind::Create(CreateKind::File));
    event.paths.push(file.clone());
    assert!(watcher::apply_event(&database_path, &root, &event).unwrap());
    let connection = database::open_database(&database_path).unwrap();
    let result = file_repository::search_entries(
        &connection,
        &SearchQuery {
            root_path: root.to_string_lossy().into(),
            query: String::new(),
            extension: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Name,
            sort_direction: SearchSortDirection::Asc,
            page: 1,
            page_size: 50,
        },
    )
    .unwrap();
    assert_eq!(result.entries.len(), 1);
    assert!(
        output
            .entries
            .iter()
            .any(|entry| entry.kind == EntryKind::File)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn watcher_ignores_paths_outside_root() {
    let root = PathBuf::from("C:/authorized");
    let outside = PathBuf::from("C:/outside/file.txt");
    let mut event = Event::new(EventKind::Create(CreateKind::File));
    event.paths.push(outside);
    assert!(!watcher::is_authorized_path(&root, &event.paths[0]));
}

#[test]
fn watcher_rename_replaces_the_old_index_path() {
    let root =
        std::env::temp_dir().join(format!("ai-file-organizer-rename-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let source = root.join("before.txt");
    let destination = root.join("after.txt");
    fs::write(&source, "hello").unwrap();
    let database_path = std::env::temp_dir().join(format!(
        "ai-file-organizer-rename-{}.sqlite",
        std::process::id()
    ));
    let _ = fs::remove_file(&database_path);
    let mut connection = database::open_database(&database_path).unwrap();
    let root_id = file_repository::upsert_scan_root(
        &connection,
        &file_repository::ScanRoot {
            path: root.to_string_lossy().into(),
            normalized_path: root.to_string_lossy().into(),
            created_at: "now".into(),
            last_scan_at: None,
        },
    )
    .unwrap();
    let entry = scanner::entry_from_path(&source).unwrap();
    file_repository::replace_entries_for_root(&mut connection, root_id, &[entry], &[], "now")
        .unwrap();
    fs::rename(&source, &destination).unwrap();
    let mut event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)));
    event.paths.extend([source, destination]);
    watcher::apply_event(&database_path, &root, &event).unwrap();
    let result = file_repository::search_entries(
        &connection,
        &SearchQuery {
            root_path: root.to_string_lossy().into(),
            query: String::new(),
            extension: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Name,
            sort_direction: SearchSortDirection::Asc,
            page: 1,
            page_size: 50,
        },
    )
    .unwrap();
    assert_eq!(
        result
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["after.txt"]
    );
    drop(connection);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(database_path);
}

#[test]
fn watcher_directory_removal_clears_descendant_entries() {
    let root = std::env::temp_dir().join(format!(
        "ai-file-organizer-remove-dir-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let nested = root.join("nested");
    let file = nested.join("inside.txt");
    fs::create_dir_all(&nested).unwrap();
    fs::write(&file, "hello").unwrap();
    let database_path = std::env::temp_dir().join(format!(
        "ai-file-organizer-remove-dir-{}.sqlite",
        std::process::id()
    ));
    let _ = fs::remove_file(&database_path);
    let mut connection = database::open_database(&database_path).unwrap();
    let root_id = file_repository::upsert_scan_root(
        &connection,
        &file_repository::ScanRoot {
            path: root.to_string_lossy().into(),
            normalized_path: root.to_string_lossy().into(),
            created_at: "now".into(),
            last_scan_at: None,
        },
    )
    .unwrap();
    let entry = scanner::entry_from_path(&file).unwrap();
    file_repository::replace_entries_for_root(&mut connection, root_id, &[entry], &[], "now")
        .unwrap();
    fs::remove_dir_all(&nested).unwrap();
    let mut event = Event::new(EventKind::Remove(RemoveKind::Folder));
    event.paths.push(nested);
    watcher::apply_event(&database_path, &root, &event).unwrap();
    let result = file_repository::search_entries(
        &connection,
        &SearchQuery {
            root_path: root.to_string_lossy().into(),
            query: String::new(),
            extension: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Name,
            sort_direction: SearchSortDirection::Asc,
            page: 1,
            page_size: 50,
        },
    )
    .unwrap();
    assert_eq!(result.total, 0);
    drop(connection);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(database_path);
}
