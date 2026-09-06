CREATE TABLE IF NOT EXISTS operation_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id TEXT NOT NULL,
  action TEXT NOT NULL,
  operation TEXT NOT NULL,
  source_path TEXT NOT NULL,
  target_path TEXT NOT NULL,
  status TEXT NOT NULL,
  reason TEXT,
  created_at TEXT NOT NULL,
  snapshot_kind TEXT,
  snapshot_size INTEGER,
  snapshot_modified_ms INTEGER,
  snapshot_file_identity TEXT,
  snapshot_volume_id TEXT,
  reverses_id INTEGER REFERENCES operation_history(id)
);

CREATE INDEX IF NOT EXISTS idx_operation_history_created
  ON operation_history(created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_operation_history_batch
  ON operation_history(batch_id);
CREATE INDEX IF NOT EXISTS idx_operation_history_reverses
  ON operation_history(reverses_id);
