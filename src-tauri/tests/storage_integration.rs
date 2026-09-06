use ai_file_organizer_lib::models::file_entry::{EntryKind, FileEntry};
use ai_file_organizer_lib::storage::database::open_memory_database;
use ai_file_organizer_lib::storage::file_repository::{
    ScanRoot, replace_entries_for_root, upsert_scan_root,
};
use std::path::PathBuf;

#[test]
fn repeated_index_replaces_entries_without_duplicates() {
    let mut database = open_memory_database().unwrap();
    let root = ScanRoot {
        path: "root".into(),
        normalized_path: "root".into(),
        created_at: "now".into(),
        last_scan_at: None,
    };
    let root_id = upsert_scan_root(&database, &root).unwrap();
    let entry = FileEntry {
        normalized_path: PathBuf::from("root/file.txt"),
        name: "file.txt".into(),
        extension: Some("txt".into()),
        kind: EntryKind::File,
        size: 3,
        modified_ms: None,
        file_identity: None,
    };
    replace_entries_for_root(
        &mut database,
        root_id,
        std::slice::from_ref(&entry),
        &[],
        "now",
    )
    .unwrap();
    let delta = replace_entries_for_root(
        &mut database,
        root_id,
        std::slice::from_ref(&entry),
        &[],
        "later",
    )
    .unwrap();
    assert_eq!(delta.added, 0);
    assert_eq!(delta.updated, 0);
    assert_eq!(delta.removed, 0);
}
