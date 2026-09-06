CREATE TABLE IF NOT EXISTS scan_roots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL,
  normalized_path TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  last_scan_at TEXT
);

CREATE TABLE IF NOT EXISTS file_entries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,
  normalized_path TEXT NOT NULL,
  name TEXT NOT NULL,
  extension TEXT,
  kind TEXT NOT NULL,
  size INTEGER NOT NULL,
  modified_ms INTEGER,
  file_identity TEXT,
  last_seen_at TEXT NOT NULL,
  UNIQUE(root_id, normalized_path)
);

CREATE TABLE IF NOT EXISTS scan_errors (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  kind TEXT NOT NULL,
  message TEXT NOT NULL,
  occurred_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_file_entries_root ON file_entries(root_id);
CREATE INDEX IF NOT EXISTS idx_file_entries_name ON file_entries(root_id, name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_file_entries_path ON file_entries(root_id, normalized_path COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_file_entries_extension ON file_entries(root_id, extension COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_file_entries_size ON file_entries(root_id, size);
CREATE INDEX IF NOT EXISTS idx_file_entries_modified ON file_entries(root_id, modified_ms);
