use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Result, params, params_from_iter};
use std::collections::{HashMap, HashSet};

use crate::models::file_entry::{EntryKind, FileEntry};
use crate::models::scan::{IndexDelta, ScanError};
use crate::models::search::{
    ExplorerNode, SearchEntry, SearchQuery, SearchResult, SearchSortDirection, SearchSortField,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoot {
    pub path: String,
    pub normalized_path: String,
    pub created_at: String,
    pub last_scan_at: Option<String>,
}

pub type ScanRootId = i64;

pub fn upsert_scan_root(connection: &Connection, root: &ScanRoot) -> Result<ScanRootId> {
    connection.execute(
        "INSERT INTO scan_roots(path, normalized_path, created_at, last_scan_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(normalized_path) DO UPDATE SET path = excluded.path, last_scan_at = excluded.last_scan_at",
        params![root.path, root.normalized_path, root.created_at, root.last_scan_at],
    )?;
    connection.query_row(
        "SELECT id FROM scan_roots WHERE normalized_path = ?1",
        params![root.normalized_path],
        |row| row.get(0),
    )
}

pub fn replace_entries_for_root(
    connection: &mut Connection,
    root_id: ScanRootId,
    entries: &[FileEntry],
    errors: &[ScanError],
    scanned_at: &str,
) -> Result<IndexDelta> {
    let transaction = connection.transaction()?;
    let mut previous = HashMap::new();
    {
        let mut statement = transaction.prepare("SELECT normalized_path, name, extension, kind, size, modified_ms, file_identity FROM file_entries WHERE root_id = ?1")?;
        let rows = statement.query_map(params![root_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ),
            ))
        })?;
        for row in rows {
            let (path, value) = row?;
            previous.insert(path, value);
        }
    }
    let current_paths: HashSet<String> = entries
        .iter()
        .map(|entry| entry.normalized_path.to_string_lossy().to_string())
        .collect();
    let added = entries
        .iter()
        .filter(|entry| {
            !previous.contains_key(&entry.normalized_path.to_string_lossy().to_string())
        })
        .count() as i64;
    let updated = entries
        .iter()
        .filter(|entry| {
            let path = entry.normalized_path.to_string_lossy().to_string();
            previous.get(&path).is_some_and(|old| {
                old != &(
                    entry.name.clone(),
                    entry.extension.clone(),
                    kind_name(&entry.kind).to_string(),
                    entry.size as i64,
                    entry.modified_ms,
                    entry.file_identity.clone(),
                )
            })
        })
        .count() as i64;
    let removed = previous
        .keys()
        .filter(|path| !current_paths.contains(*path))
        .count() as i64;
    transaction.execute(
        "DELETE FROM file_entries WHERE root_id = ?1",
        params![root_id],
    )?;
    transaction.execute(
        "DELETE FROM scan_errors WHERE root_id = ?1",
        params![root_id],
    )?;
    for entry in entries {
        transaction.execute(
            "INSERT INTO file_entries(root_id, normalized_path, name, extension, kind, size, modified_ms, file_identity, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                root_id,
                entry.normalized_path.to_string_lossy(),
                entry.name,
                entry.extension,
                kind_name(&entry.kind),
                entry.size as i64,
                entry.modified_ms,
                entry.file_identity,
                scanned_at,
            ],
        )?;
    }
    for error in errors {
        transaction.execute(
            "INSERT INTO scan_errors(root_id, path, kind, message, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![root_id, error.path, error.kind, error.message, scanned_at],
        )?;
    }
    transaction.execute(
        "UPDATE scan_roots SET last_scan_at = ?1 WHERE id = ?2",
        params![scanned_at, root_id],
    )?;
    transaction.commit()?;
    Ok(IndexDelta {
        added,
        updated,
        removed,
    })
}

pub fn read_index_status(
    connection: &Connection,
    root: &Path,
) -> Result<Option<(i64, Option<String>)>> {
    connection.query_row(
        "SELECT COUNT(file_entries.id), scan_roots.last_scan_at FROM scan_roots LEFT JOIN file_entries ON file_entries.root_id = scan_roots.id WHERE scan_roots.normalized_path = ?1 GROUP BY scan_roots.id",
        params![root.to_string_lossy()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()
}

pub fn read_latest_index_status(
    connection: &Connection,
) -> Result<Option<(String, i64, Option<String>)>> {
    connection
        .query_row(
            "SELECT scan_roots.normalized_path, COUNT(file_entries.id), scan_roots.last_scan_at
             FROM scan_roots
             LEFT JOIN file_entries ON file_entries.root_id = scan_roots.id
             GROUP BY scan_roots.id
             ORDER BY CAST(COALESCE(scan_roots.last_scan_at, scan_roots.created_at) AS INTEGER) DESC, scan_roots.id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
}

pub fn reset_file_index(connection: &Connection) -> Result<()> {
    connection.execute_batch("DELETE FROM file_entries; DELETE FROM scan_errors;")
}

pub fn upsert_entry_for_root(
    connection: &Connection,
    root_id: ScanRootId,
    entry: &FileEntry,
    seen_at: &str,
) -> Result<()> {
    connection.execute(
        "INSERT INTO file_entries(root_id, normalized_path, name, extension, kind, size, modified_ms, file_identity, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(root_id, normalized_path) DO UPDATE SET
           name = excluded.name, extension = excluded.extension, kind = excluded.kind,
           size = excluded.size, modified_ms = excluded.modified_ms,
           file_identity = excluded.file_identity, last_seen_at = excluded.last_seen_at",
        params![
            root_id,
            entry.normalized_path.to_string_lossy(),
            entry.name,
            entry.extension,
            kind_name(&entry.kind),
            entry.size as i64,
            entry.modified_ms,
            entry.file_identity,
            seen_at,
        ],
    )?;
    connection.execute(
        "UPDATE scan_roots SET last_scan_at = ?1 WHERE id = ?2",
        params![seen_at, root_id],
    )?;
    Ok(())
}

pub fn remove_entry_for_root(
    connection: &Connection,
    root_id: ScanRootId,
    normalized_path: &str,
    seen_at: &str,
) -> Result<()> {
    connection.execute(
        "DELETE FROM file_entries WHERE root_id = ?1 AND normalized_path = ?2",
        params![root_id, normalized_path],
    )?;
    connection.execute(
        "UPDATE scan_roots SET last_scan_at = ?1 WHERE id = ?2",
        params![seen_at, root_id],
    )?;
    Ok(())
}

pub fn remove_entries_at_or_below_path(
    connection: &Connection,
    root_id: ScanRootId,
    path: &Path,
    seen_at: &str,
) -> Result<usize> {
    let mut statement =
        connection.prepare("SELECT id, normalized_path FROM file_entries WHERE root_id = ?1")?;
    let rows = statement.query_map(params![root_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut ids = Vec::new();
    for row in rows {
        let (id, stored_path) = row?;
        if path_matches_or_descends(&stored_path, path) {
            ids.push(id);
        }
    }
    drop(statement);
    for id in &ids {
        connection.execute("DELETE FROM file_entries WHERE id = ?1", params![id])?;
    }
    connection.execute(
        "UPDATE scan_roots SET last_scan_at = ?1 WHERE id = ?2",
        params![seen_at, root_id],
    )?;
    Ok(ids.len())
}

pub fn scan_root_id(connection: &Connection, normalized_path: &str) -> Result<Option<ScanRootId>> {
    connection
        .query_row(
            "SELECT id FROM scan_roots WHERE normalized_path = ?1",
            params![normalized_path],
            |row| row.get(0),
        )
        .optional()
}

pub fn search_entries(connection: &Connection, query: &SearchQuery) -> Result<SearchResult> {
    let root_id = connection
        .query_row(
            "SELECT id FROM scan_roots WHERE normalized_path = ?1",
            params![query.root_path],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(root_id) = root_id else {
        return Ok(SearchResult {
            entries: Vec::new(),
            total: 0,
            page: query.page,
            page_size: query.page_size,
            total_pages: 0,
        });
    };

    let (where_sql, values) = search_predicates(root_id, query);
    let total_sql = format!("SELECT COUNT(*) FROM file_entries WHERE {where_sql}");
    let total: i64 = connection.query_row(
        &total_sql,
        params_from_iter(values.iter().cloned()),
        |row| row.get(0),
    )?;
    let offset = (query.page - 1) * query.page_size;
    let order_sql = order_clause(query);
    let entries_sql = format!(
        "SELECT id, normalized_path, name, extension, kind, size, modified_ms
         FROM file_entries WHERE {where_sql} ORDER BY {order_sql}, id ASC LIMIT ? OFFSET ?"
    );
    let mut entry_values = values;
    entry_values.push(Value::Integer(query.page_size));
    entry_values.push(Value::Integer(offset));
    let mut statement = connection.prepare(&entries_sql)?;
    let rows = statement.query_map(params_from_iter(entry_values), |row| {
        Ok(SearchEntry {
            id: row.get(0)?,
            normalized_path: row.get(1)?,
            name: row.get(2)?,
            extension: row.get(3)?,
            kind: row.get(4)?,
            size: row.get(5)?,
            modified_ms: row.get(6)?,
        })
    })?;
    let entries = rows.collect::<Result<Vec<_>>>()?;
    Ok(SearchResult {
        entries,
        total,
        page: query.page,
        page_size: query.page_size,
        total_pages: if total == 0 {
            0
        } else {
            (total + query.page_size - 1) / query.page_size
        },
    })
}

pub fn read_explorer_children(
    connection: &Connection,
    root_path: &str,
    parent_path: &str,
) -> Result<Vec<ExplorerNode>> {
    let Some(root_id) = scan_root_id(connection, root_path)? else {
        return Ok(Vec::new());
    };
    let parent = parent_path.replace('\\', "/");
    let mut statement = connection.prepare(
        "WITH normalized_entries AS (
           SELECT id, normalized_path, name, extension, kind, size, modified_ms,
                  replace(normalized_path, char(92), '/') AS slash_path
           FROM file_entries
           WHERE root_id = ?1
         )
         SELECT entry.id, entry.normalized_path, entry.name, entry.extension, entry.kind,
                entry.size, entry.modified_ms,
                CASE WHEN entry.kind = 'directory' AND EXISTS (
                  SELECT 1 FROM normalized_entries child
                  WHERE substr(child.slash_path, 1, length(entry.slash_path) + 1) = entry.slash_path || '/'
                    AND instr(substr(child.slash_path, length(entry.slash_path) + 2), '/') = 0
                ) THEN 1 ELSE 0 END AS has_children
         FROM normalized_entries entry
         WHERE substr(entry.slash_path, 1, length(?2) + 1) = ?2 || '/'
           AND instr(substr(entry.slash_path, length(?2) + 2), '/') = 0
         ORDER BY CASE WHEN entry.kind = 'directory' THEN 0 ELSE 1 END,
                  unicode_lower(entry.name), entry.id",
    )?;
    let rows = statement.query_map(params![root_id, parent], |row| {
        Ok(ExplorerNode {
            id: row.get(0)?,
            normalized_path: row.get(1)?,
            name: row.get(2)?,
            extension: row.get(3)?,
            kind: row.get(4)?,
            size: row.get(5)?,
            modified_ms: row.get(6)?,
            has_children: row.get(7)?,
        })
    })?;
    rows.collect::<Result<Vec<_>>>()
}

pub fn resolve_indexed_file(
    connection: &Connection,
    root_path: &str,
    file_path: &str,
) -> Result<Option<ExplorerNode>> {
    Ok(resolve_indexed_node(connection, root_path, file_path)?.filter(|node| node.kind == "file"))
}

pub fn resolve_indexed_node(
    connection: &Connection,
    root_path: &str,
    entry_path: &str,
) -> Result<Option<ExplorerNode>> {
    let Some(root_id) = scan_root_id(connection, root_path)? else {
        return Ok(None);
    };
    connection
        .query_row(
            "SELECT id, normalized_path, name, extension, kind, size, modified_ms
             FROM file_entries WHERE root_id = ?1 AND normalized_path = ?2",
            params![root_id, entry_path],
            |row| {
                Ok(ExplorerNode {
                    id: row.get(0)?,
                    normalized_path: row.get(1)?,
                    name: row.get(2)?,
                    extension: row.get(3)?,
                    kind: row.get(4)?,
                    size: row.get(5)?,
                    modified_ms: row.get(6)?,
                    has_children: false,
                })
            },
        )
        .optional()
}

fn search_predicates(root_id: i64, query: &SearchQuery) -> (String, Vec<Value>) {
    let mut predicates = vec!["root_id = ?".to_string()];
    let mut values = vec![Value::Integer(root_id)];
    if !query.query.is_empty() {
        predicates
            .push("(unicode_lower(name) LIKE ? OR unicode_lower(normalized_path) LIKE ?)".into());
        let pattern = Value::Text(format!("%{}%", query.query));
        values.push(pattern.clone());
        values.push(pattern);
    }
    if let Some(extension) = &query.extension {
        predicates.push("unicode_lower(COALESCE(extension, '')) = ?".into());
        values.push(Value::Text(extension.clone()));
    }
    if let Some(min_size) = query.min_size {
        predicates.push("size >= ?".into());
        values.push(Value::Integer(min_size));
    }
    if let Some(max_size) = query.max_size {
        predicates.push("size <= ?".into());
        values.push(Value::Integer(max_size));
    }
    if let Some(modified_after) = query.modified_after {
        predicates.push("modified_ms >= ?".into());
        values.push(Value::Integer(modified_after));
    }
    if let Some(modified_before) = query.modified_before {
        predicates.push("modified_ms <= ?".into());
        values.push(Value::Integer(modified_before));
    }
    (predicates.join(" AND "), values)
}

fn order_clause(query: &SearchQuery) -> String {
    let column = match query.sort_by {
        SearchSortField::Name => "unicode_lower(name)",
        SearchSortField::Path => "unicode_lower(normalized_path)",
        SearchSortField::Extension => "unicode_lower(extension)",
        SearchSortField::Size => "size",
        SearchSortField::Modified => "modified_ms",
    };
    let direction = match query.sort_direction {
        SearchSortDirection::Asc => "ASC",
        SearchSortDirection::Desc => "DESC",
    };
    format!("{column} {direction} NULLS LAST")
}

fn kind_name(kind: &EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
        EntryKind::Symlink => "symlink",
        EntryKind::Junction => "junction",
    }
}

fn path_matches_or_descends(stored_path: &str, event_path: &Path) -> bool {
    let stored = Path::new(stored_path);
    if stored == event_path || stored.starts_with(event_path) {
        return true;
    }
    #[cfg(windows)]
    {
        let stored = stored_path.to_lowercase();
        let event = event_path.to_string_lossy().to_lowercase();
        let event = event.trim_end_matches(['\\', '/']);
        return stored == event
            || stored.starts_with(&format!("{event}\\"))
            || stored.starts_with(&format!("{event}/"));
    }
    #[cfg(not(windows))]
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::file_entry::EntryKind;
    use crate::storage::database::open_memory_database;
    use std::path::PathBuf;

    #[test]
    fn replacing_entries_is_unique_and_reports_removed_items() {
        let mut connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "root".into(),
            normalized_path: "root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let first = FileEntry {
            normalized_path: PathBuf::from("root/a.txt"),
            name: "a.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        replace_entries_for_root(&mut connection, root_id, &[first.clone()], &[], "now").unwrap();
        let delta = replace_entries_for_root(&mut connection, root_id, &[], &[], "later").unwrap();
        assert_eq!(delta.removed, 1);
        assert_eq!(
            read_index_status(&connection, Path::new("root"))
                .unwrap()
                .unwrap()
                .0,
            0
        );
    }

    #[test]
    fn reset_does_not_drop_other_tables() {
        let connection = open_memory_database().unwrap();
        connection
            .execute(
                "CREATE TABLE other_persistent_data(id INTEGER PRIMARY KEY)",
                [],
            )
            .unwrap();
        connection
            .execute("INSERT INTO other_persistent_data(id) VALUES (1)", [])
            .unwrap();
        reset_file_index(&connection).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM other_persistent_data", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn search_entries_filters_sorts_and_pages() {
        let mut connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "root".into(),
            normalized_path: "root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let entries = [
            FileEntry {
                normalized_path: PathBuf::from("root/Zeta.PDF"),
                name: "Zeta.PDF".into(),
                extension: Some("PDF".into()),
                kind: EntryKind::File,
                size: 20,
                modified_ms: Some(20),
                file_identity: None,
            },
            FileEntry {
                normalized_path: PathBuf::from("root/alpha.pdf"),
                name: "alpha.pdf".into(),
                extension: Some("pdf".into()),
                kind: EntryKind::File,
                size: 10,
                modified_ms: Some(10),
                file_identity: None,
            },
            FileEntry {
                normalized_path: PathBuf::from("root/note.txt"),
                name: "note.txt".into(),
                extension: Some("txt".into()),
                kind: EntryKind::File,
                size: 5,
                modified_ms: Some(5),
                file_identity: None,
            },
        ];
        replace_entries_for_root(&mut connection, root_id, &entries, &[], "now").unwrap();
        let query = SearchQuery {
            root_path: "root".into(),
            query: "PDF".into(),
            extension: Some("pdf".into()),
            min_size: Some(10),
            max_size: None,
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Name,
            sort_direction: SearchSortDirection::Asc,
            page: 1,
            page_size: 1,
        };
        let query = crate::services::search::normalize_query(&query).unwrap();
        let result = search_entries(&connection, &query).unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.total_pages, 2);
        assert_eq!(result.entries[0].name, "alpha.pdf");
    }

    #[test]
    fn incremental_entry_updates_and_removes_one_path() {
        let connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "root".into(),
            normalized_path: "root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let entry = FileEntry {
            normalized_path: PathBuf::from("root/a.txt"),
            name: "a.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: Some(1),
            file_identity: None,
        };
        upsert_entry_for_root(&connection, root_id, &entry, "now").unwrap();
        let updated = FileEntry {
            size: 2,
            modified_ms: Some(2),
            ..entry.clone()
        };
        upsert_entry_for_root(&connection, root_id, &updated, "later").unwrap();
        assert_eq!(
            search_entries(
                &connection,
                &SearchQuery {
                    root_path: "root".into(),
                    query: String::new(),
                    extension: None,
                    min_size: None,
                    max_size: None,
                    modified_after: None,
                    modified_before: None,
                    sort_by: SearchSortField::Name,
                    sort_direction: SearchSortDirection::Asc,
                    page: 1,
                    page_size: 50
                }
            )
            .unwrap()
            .entries[0]
                .size,
            2
        );
        remove_entry_for_root(&connection, root_id, "root/a.txt", "latest").unwrap();
        assert_eq!(
            search_entries(
                &connection,
                &SearchQuery {
                    root_path: "root".into(),
                    query: String::new(),
                    extension: None,
                    min_size: None,
                    max_size: None,
                    modified_after: None,
                    modified_before: None,
                    sort_by: SearchSortField::Name,
                    sort_direction: SearchSortDirection::Asc,
                    page: 1,
                    page_size: 50
                }
            )
            .unwrap()
            .total,
            0
        );
    }

    #[test]
    fn removing_a_directory_path_removes_descendant_entries() {
        let connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "C:/root".into(),
            normalized_path: "C:/root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let entry = FileEntry {
            normalized_path: PathBuf::from("C:/root/nested/file.txt"),
            name: "file.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        upsert_entry_for_root(&connection, root_id, &entry, "now").unwrap();
        let removed = remove_entries_at_or_below_path(
            &connection,
            root_id,
            Path::new("C:/root/nested"),
            "later",
        )
        .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            search_entries(
                &connection,
                &SearchQuery {
                    root_path: "C:/root".into(),
                    query: String::new(),
                    extension: None,
                    min_size: None,
                    max_size: None,
                    modified_after: None,
                    modified_before: None,
                    sort_by: SearchSortField::Name,
                    sort_direction: SearchSortDirection::Asc,
                    page: 1,
                    page_size: 50
                }
            )
            .unwrap()
            .total,
            0
        );
    }

    #[test]
    fn reads_the_most_recent_persisted_index_root() {
        let mut connection = open_memory_database().unwrap();
        let older = ScanRoot {
            path: "C:/older".into(),
            normalized_path: "C:/older".into(),
            created_at: "1".into(),
            last_scan_at: Some("10".into()),
        };
        let newer = ScanRoot {
            path: "C:/newer".into(),
            normalized_path: "C:/newer".into(),
            created_at: "2".into(),
            last_scan_at: Some("20".into()),
        };
        let older_id = upsert_scan_root(&connection, &older).unwrap();
        let newer_id = upsert_scan_root(&connection, &newer).unwrap();
        let entry = FileEntry {
            normalized_path: PathBuf::from("C:/newer/file.txt"),
            name: "file.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        replace_entries_for_root(&mut connection, newer_id, &[entry], &[], "20").unwrap();
        assert_ne!(older_id, newer_id);
        assert_eq!(
            read_latest_index_status(&connection).unwrap(),
            Some(("C:/newer".into(), 1, Some("20".into())))
        );
    }

    #[test]
    fn explorer_children_are_direct_sorted_and_links_are_leaves() {
        let mut connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "C:/root".into(),
            normalized_path: "C:/root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let entries = [
            FileEntry {
                normalized_path: PathBuf::from("C:/root/z.txt"),
                name: "z.txt".into(),
                extension: Some("txt".into()),
                kind: EntryKind::File,
                size: 2,
                modified_ms: Some(2),
                file_identity: None,
            },
            FileEntry {
                normalized_path: PathBuf::from("C:/root/Alpha"),
                name: "Alpha".into(),
                extension: None,
                kind: EntryKind::Directory,
                size: 0,
                modified_ms: None,
                file_identity: None,
            },
            FileEntry {
                normalized_path: PathBuf::from("C:/root/Alpha/a.md"),
                name: "a.md".into(),
                extension: Some("md".into()),
                kind: EntryKind::File,
                size: 1,
                modified_ms: Some(1),
                file_identity: None,
            },
            FileEntry {
                normalized_path: PathBuf::from("C:/root/link"),
                name: "link".into(),
                extension: None,
                kind: EntryKind::Symlink,
                size: 0,
                modified_ms: None,
                file_identity: None,
            },
        ];
        replace_entries_for_root(&mut connection, root_id, &entries, &[], "now").unwrap();

        let root_children = read_explorer_children(&connection, "C:/root", "C:/root").unwrap();
        assert_eq!(
            root_children
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "link", "z.txt"]
        );
        assert!(root_children[0].has_children);
        assert!(!root_children[1].has_children);
        let nested = read_explorer_children(&connection, "C:/root", "C:/root/Alpha").unwrap();
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].name, "a.md");
    }

    #[test]
    fn resolves_only_an_indexed_file_for_the_requested_root() {
        let connection = open_memory_database().unwrap();
        let root = ScanRoot {
            path: "C:/root".into(),
            normalized_path: "C:/root".into(),
            created_at: "now".into(),
            last_scan_at: None,
        };
        let root_id = upsert_scan_root(&connection, &root).unwrap();
        let entry = FileEntry {
            normalized_path: PathBuf::from("C:/root/a.txt"),
            name: "a.txt".into(),
            extension: Some("txt".into()),
            kind: EntryKind::File,
            size: 1,
            modified_ms: None,
            file_identity: None,
        };
        upsert_entry_for_root(&connection, root_id, &entry, "now").unwrap();

        assert_eq!(
            resolve_indexed_file(&connection, "C:/root", "C:/root/a.txt")
                .unwrap()
                .unwrap()
                .name,
            "a.txt"
        );
        assert!(
            resolve_indexed_file(&connection, "C:/root", "C:/other/a.txt")
                .unwrap()
                .is_none()
        );
    }
}
