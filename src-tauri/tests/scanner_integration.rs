use ai_file_organizer_lib::services::scanner::scan_directory;
use std::fs;

#[test]
fn scanning_is_read_only_and_reports_incremental_metadata() {
    let root =
        std::env::temp_dir().join(format!("ai-organizer-integration-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("nested/report.md"), "report").unwrap();
    let before = fs::read_dir(&root).unwrap().count();

    let result = scan_directory(&root).unwrap();

    assert_eq!(result.entries.len(), 2);
    assert!(result.entries.iter().any(|entry| entry.name == "nested"));
    assert!(result.entries.iter().any(|entry| entry.name == "report.md"));
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
    let _ = fs::remove_dir_all(root);
}
