use std::path::{Path, PathBuf};

pub fn database_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("index.sqlite3")
}
