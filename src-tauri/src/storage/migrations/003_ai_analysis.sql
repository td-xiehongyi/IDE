CREATE TABLE IF NOT EXISTS ai_categories (
    root_path TEXT NOT NULL,
    category_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    directory_path TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    PRIMARY KEY (root_path, category_id),
    UNIQUE (root_path, directory_path)
);

CREATE TABLE IF NOT EXISTS ai_analysis_results (
    id TEXT PRIMARY KEY,
    batch_id TEXT NOT NULL,
    root_path TEXT NOT NULL,
    source_path TEXT NOT NULL,
    content_fingerprint TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    summary TEXT NOT NULL,
    keywords_json TEXT NOT NULL,
    suggested_filename TEXT NOT NULL,
    category_id TEXT,
    confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    reason TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'expired')),
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_results_batch ON ai_analysis_results(batch_id, created_at, id);
CREATE INDEX IF NOT EXISTS idx_ai_results_source ON ai_analysis_results(root_path, source_path, created_at);
