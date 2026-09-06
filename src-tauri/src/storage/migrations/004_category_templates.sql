CREATE TABLE IF NOT EXISTS ai_category_templates (
    template_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version >= 1),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_category_template_items (
    template_id TEXT NOT NULL REFERENCES ai_category_templates(template_id) ON DELETE CASCADE,
    category_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    default_enabled INTEGER NOT NULL CHECK (default_enabled IN (0, 1)),
    PRIMARY KEY (template_id, category_id)
);

CREATE TABLE IF NOT EXISTS ai_root_category_templates (
    root_path TEXT PRIMARY KEY,
    template_id TEXT NOT NULL,
    template_version INTEGER NOT NULL CHECK (template_version >= 1),
    FOREIGN KEY (template_id) REFERENCES ai_category_templates(template_id) ON DELETE RESTRICT
);

ALTER TABLE ai_analysis_results ADD COLUMN template_id TEXT;
ALTER TABLE ai_analysis_results ADD COLUMN template_version INTEGER;
