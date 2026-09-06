use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::storage::{app_paths, database, file_repository};

#[derive(Default)]
pub struct WatcherState {
    watcher: Mutex<Option<RecommendedWatcher>>,
    root: Mutex<Option<PathBuf>>,
}

impl WatcherState {
    pub fn is_current_root(&self, root: &Path) -> Result<bool, String> {
        let current = self
            .root
            .lock()
            .map_err(|_| "监听器状态不可用。".to_string())?;
        Ok(current.as_deref() == Some(root))
    }

    pub fn replace<R: Runtime + 'static>(
        &self,
        app: AppHandle<R>,
        root: &Path,
    ) -> Result<(), String> {
        self.stop()?;
        let root = root.to_path_buf();
        let database_path = app_paths::database_path(
            &app.path()
                .app_data_dir()
                .map_err(|error| error.to_string())?,
        );
        let callback_root = root.clone();
        let callback_app = app.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                match result {
                    Ok(event) => match apply_event(&database_path, &callback_root, &event) {
                        Ok(true) => {
                            let _ = callback_app.emit("files://index-changed", ());
                        }
                        Ok(false) => {}
                        Err(error) => {
                            let _ = callback_app.emit("files://watcher-error", error);
                        }
                    },
                    Err(error) => {
                        let _ = callback_app.emit("files://watcher-error", error.to_string());
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        *self
            .watcher
            .lock()
            .map_err(|_| "监听器状态不可用。".to_string())? = Some(watcher);
        *self
            .root
            .lock()
            .map_err(|_| "监听器状态不可用。".to_string())? = Some(root);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        *self
            .watcher
            .lock()
            .map_err(|_| "监听器状态不可用。".to_string())? = None;
        *self
            .root
            .lock()
            .map_err(|_| "监听器状态不可用。".to_string())? = None;
        Ok(())
    }
}

pub fn apply_event(database_path: &Path, root: &Path, event: &Event) -> Result<bool, String> {
    let mut seen = HashSet::new();
    let paths = event
        .paths
        .iter()
        .filter(|path| is_authorized_path(root, path))
        .filter(|path| seen.insert((*path).clone()))
        .cloned()
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Ok(false);
    }
    let connection = database::open_database(database_path).map_err(|error| error.to_string())?;
    let root_id = file_repository::scan_root_id(&connection, &root.to_string_lossy())
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "监听目录尚未建立索引。".to_string())?;
    let now = now_string();
    for (index, path) in paths.iter().enumerate() {
        let removes_path = matches!(event.kind, EventKind::Remove(_))
            || matches!(
                event.kind,
                EventKind::Modify(ModifyKind::Name(RenameMode::From))
            )
            || (matches!(
                event.kind,
                EventKind::Modify(ModifyKind::Name(RenameMode::Both))
            ) && index == 0);
        if removes_path {
            file_repository::remove_entries_at_or_below_path(&connection, root_id, path, &now)
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(entry) = crate::services::scanner::entry_from_path(path) {
            file_repository::upsert_entry_for_root(&connection, root_id, &entry, &now)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(true)
}

pub fn is_authorized_path(root: &Path, path: &Path) -> bool {
    path == root || path.strip_prefix(root).is_ok()
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_accepts_only_paths_inside_authorized_root() {
        let root = Path::new("C:/Docs");
        assert!(is_authorized_path(root, Path::new("C:/Docs/report.txt")));
        assert!(!is_authorized_path(root, Path::new("C:/Other/report.txt")));
    }
}
