use crate::models::search::{
    ExplorerChildrenQuery, ExplorerNode, IndexedFileQuery, SearchQuery, SearchResult,
};

pub const DEFAULT_PAGE_SIZE: i64 = 50;
pub const MAX_PAGE_SIZE: i64 = 200;

pub fn search(
    connection: &rusqlite::Connection,
    query: &SearchQuery,
) -> Result<SearchResult, String> {
    let normalized = normalize_query(query)?;
    crate::storage::file_repository::search_entries(connection, &normalized)
        .map_err(|error| error.to_string())
}

pub fn normalize_query(query: &SearchQuery) -> Result<SearchQuery, String> {
    if query.page < 1 {
        return Err("页码必须从 1 开始。".into());
    }
    if query.page_size < 1 || query.page_size > MAX_PAGE_SIZE {
        return Err(format!("页大小必须在 1 到 {MAX_PAGE_SIZE} 之间。"));
    }
    if query.min_size.is_some_and(|value| value < 0)
        || query.max_size.is_some_and(|value| value < 0)
    {
        return Err("文件大小不能为负数。".into());
    }
    if query
        .min_size
        .zip(query.max_size)
        .is_some_and(|(min, max)| min > max)
    {
        return Err("最小文件大小不能大于最大文件大小。".into());
    }
    if query
        .modified_after
        .zip(query.modified_before)
        .is_some_and(|(after, before)| after > before)
    {
        return Err("修改时间范围无效。".into());
    }
    let mut normalized = query.clone();
    normalized.root_path = query.root_path.trim().to_string();
    normalized.query = query.query.trim().to_lowercase();
    normalized.extension = query
        .extension
        .as_ref()
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty());
    Ok(normalized)
}

pub fn explorer_children(
    connection: &rusqlite::Connection,
    query: &ExplorerChildrenQuery,
) -> Result<Vec<ExplorerNode>, String> {
    let root = query.root_path.trim();
    if root.is_empty() {
        return Err("授权根目录不能为空。".into());
    }
    let parent = query.parent_path.as_deref().unwrap_or(root).trim();
    if parent != root {
        let node = crate::storage::file_repository::resolve_indexed_node(connection, root, parent)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "父目录不属于当前索引。".to_string())?;
        if node.kind != "directory" {
            return Err("只能展开已索引的普通目录。".into());
        }
    }
    crate::storage::file_repository::read_explorer_children(connection, root, parent)
        .map_err(|error| error.to_string())
}

pub fn indexed_file(
    connection: &rusqlite::Connection,
    query: &IndexedFileQuery,
) -> Result<Option<ExplorerNode>, String> {
    let root = query.root_path.trim();
    let file_path = query.file_path.trim();
    if root.is_empty() || file_path.is_empty() {
        return Err("授权根目录和文件路径不能为空。".into());
    }
    let entry = crate::storage::file_repository::resolve_indexed_file(connection, root, file_path)
        .map_err(|error| error.to_string())?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::models::file_entry::{EntryKind, FileEntry};
    use crate::models::search::{
        ExplorerChildrenQuery, IndexedFileQuery, SearchSortDirection, SearchSortField,
    };
    use crate::storage::database::open_memory_database;
    use crate::storage::file_repository::{ScanRoot, upsert_entry_for_root, upsert_scan_root};

    fn query() -> SearchQuery {
        SearchQuery {
            root_path: " C:/Docs ".into(),
            query: "  Report ".into(),
            extension: Some(".PDF".into()),
            min_size: Some(1),
            max_size: Some(2),
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Name,
            sort_direction: SearchSortDirection::Asc,
            page: 1,
            page_size: 50,
        }
    }

    #[test]
    fn normalizes_search_terms_and_extension() {
        let normalized = normalize_query(&query()).unwrap();
        assert_eq!(normalized.root_path, "C:/Docs");
        assert_eq!(normalized.query, "report");
        assert_eq!(normalized.extension.as_deref(), Some("pdf"));
    }

    #[test]
    fn rejects_invalid_paging_and_ranges() {
        let mut invalid = query();
        invalid.page = 0;
        assert!(normalize_query(&invalid).is_err());
        invalid = query();
        invalid.min_size = Some(3);
        assert!(normalize_query(&invalid).is_err());
    }

    #[test]
    fn explorer_service_rejects_unknown_or_non_directory_parents() {
        let connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "C:/root".into(),
            normalized_path: "C:/root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let file = FileEntry {
            normalized_path: PathBuf::from("C:/root/a.txt"),
            name: "a.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        upsert_entry_for_root(&connection, root_id, &file, "now").unwrap();

        let file_parent = ExplorerChildrenQuery {
            root_path: "C:/root".into(),
            parent_path: Some("C:/root/a.txt".into()),
        };
        assert!(explorer_children(&connection, &file_parent).is_err());
        let outside = ExplorerChildrenQuery {
            root_path: "C:/root".into(),
            parent_path: Some("C:/other".into()),
        };
        assert!(explorer_children(&connection, &outside).is_err());
    }

    #[test]
    fn indexed_file_service_returns_only_plain_files() {
        let connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "C:/root".into(),
            normalized_path: "C:/root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let file = FileEntry {
            normalized_path: PathBuf::from("C:/root/a.txt"),
            name: "a.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        upsert_entry_for_root(&connection, root_id, &file, "now").unwrap();

        let query = IndexedFileQuery {
            root_path: "C:/root".into(),
            file_path: "C:/root/a.txt".into(),
        };
        assert_eq!(
            indexed_file(&connection, &query).unwrap().unwrap().name,
            "a.txt"
        );
        let outside = IndexedFileQuery {
            root_path: "C:/root".into(),
            file_path: "C:/other/a.txt".into(),
        };
        assert!(indexed_file(&connection, &outside).unwrap().is_none());
    }
}
