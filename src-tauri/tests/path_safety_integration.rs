use ai_file_organizer_lib::services::path_policy::{is_ignored_name, normalize_root};
use std::path::Path;

#[test]
fn path_policy_rejects_files_as_scan_roots_and_applies_fixed_ignores() {
    let file = std::env::temp_dir().join(format!("ai-organizer-file-{}", std::process::id()));
    std::fs::write(&file, "not a directory").unwrap();
    assert!(normalize_root(&file).is_err());
    assert!(is_ignored_name("node_modules", true));
    assert!(is_ignored_name("draft.tmp", false));
    assert!(!is_ignored_name("important.md", false));
    assert!(Path::new(&file).exists());
    let _ = std::fs::remove_file(file);
}
