use ai_file_organizer_lib::models::file_entry::{EntryKind, FileEntry};
use ai_file_organizer_lib::models::search::{SearchQuery, SearchSortDirection, SearchSortField};
use ai_file_organizer_lib::services::search;
use ai_file_organizer_lib::storage::database::open_memory_database;
use ai_file_organizer_lib::storage::file_repository::{
    ScanRoot, replace_entries_for_root, upsert_scan_root,
};
use std::path::PathBuf;

#[test]
fn search_integration_returns_unicode_results_with_pagination() {
    let mut connection = open_memory_database().unwrap();
    let root = ScanRoot {
        path: "C:/资料".into(),
        normalized_path: "C:/资料".into(),
        created_at: "now".into(),
        last_scan_at: None,
    };
    let root_id = upsert_scan_root(&connection, &root).unwrap();
    let entries = (0..3)
        .map(|index| FileEntry {
            normalized_path: PathBuf::from(format!("C:/资料/报告-{index}.txt")),
            name: format!("报告-{index}.txt"),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: index + 1,
            modified_ms: Some(index as i64),
            file_identity: None,
        })
        .collect::<Vec<_>>();
    replace_entries_for_root(&mut connection, root_id, &entries, &[], "now").unwrap();
    let query = SearchQuery {
        root_path: "C:/资料".into(),
        query: "报告".into(),
        extension: None,
        min_size: None,
        max_size: None,
        modified_after: None,
        modified_before: None,
        sort_by: SearchSortField::Name,
        sort_direction: SearchSortDirection::Asc,
        page: 2,
        page_size: 2,
    };
    let result = search::search(&connection, &query).unwrap();
    assert_eq!(result.total, 3);
    assert_eq!(result.total_pages, 2);
    assert_eq!(result.entries.len(), 1);
}

#[test]
fn search_matches_non_ascii_case_consistently() {
    let mut connection = open_memory_database().unwrap();
    let root = ScanRoot {
        path: "C:/资料".into(),
        normalized_path: "C:/资料".into(),
        created_at: "now".into(),
        last_scan_at: None,
    };
    let root_id = upsert_scan_root(&connection, &root).unwrap();
    let entry = FileEntry {
        normalized_path: PathBuf::from("C:/资料/Ärende.txt"),
        name: "Ärende.txt".into(),
        extension: Some("TXT".into()),
        kind: EntryKind::File,
        size: 1,
        modified_ms: None,
        file_identity: None,
    };
    replace_entries_for_root(&mut connection, root_id, &[entry], &[], "now").unwrap();
    let query = SearchQuery {
        root_path: "C:/资料".into(),
        query: "ärende".into(),
        extension: Some("txt".into()),
        min_size: None,
        max_size: None,
        modified_after: None,
        modified_before: None,
        sort_by: SearchSortField::Name,
        sort_direction: SearchSortDirection::Asc,
        page: 1,
        page_size: 50,
    };
    let result = search::search(&connection, &query).unwrap();
    assert_eq!(result.total, 1);
}
