use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::models::operation::{
    OperationDraft, OperationDraftItem, OperationPreview, OperationPreviewItem,
    OperationValidationStatus,
};

use super::file_identity::{snapshot_directory, snapshot_file};
use super::path_policy::category_directory;

pub fn validate_draft(draft: &OperationDraft) -> Result<OperationPreview, String> {
    let root = normalize_directory(Path::new(&draft.root_path))?;
    if draft.items.is_empty() {
        return Err("操作草案不能为空。".into());
    }

    let mut items = Vec::with_capacity(draft.items.len());
    for (index, item) in draft.items.iter().enumerate() {
        items.push(validate_item(&root, index, item));
    }
    let can_confirm = items
        .iter()
        .all(|item| item.status == OperationValidationStatus::Valid);
    Ok(OperationPreview { can_confirm, items })
}

fn validate_item(root: &Path, index: usize, item: &OperationDraftItem) -> OperationPreviewItem {
    let operation = item.operation_type();
    let source_input = Path::new(item.source_path());
    let source_path = fs::canonicalize(source_input).unwrap_or_else(|_| source_input.to_path_buf());
    let target_path = match item {
        OperationDraftItem::Move {
            destination_directory,
            ..
        } => fs::canonicalize(destination_directory)
            .map(|directory| directory.join(source_path.file_name().unwrap_or_default()))
            .unwrap_or_else(|_| {
                Path::new(destination_directory).join(source_path.file_name().unwrap_or_default())
            }),
        OperationDraftItem::AiOrganize {
            category_id,
            new_name,
            ..
        } => category_directory(root, category_id)
            .unwrap_or_else(|_| root.join(category_id))
            .canonicalize()
            .map(|directory| directory.join(new_name))
            .unwrap_or_else(|_| root.join(category_id).join(new_name)),
        OperationDraftItem::Rename { new_name, .. }
        | OperationDraftItem::AiRename { new_name, .. } => source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(new_name),
    };

    let will_create_directory = matches!(item, OperationDraftItem::AiOrganize { .. })
        && target_path.parent().is_some_and(|parent| !parent.exists());
    match validate_item_inner(root, item, &source_path, &target_path) {
        Ok(snapshot) => OperationPreviewItem {
            index,
            operation,
            source_path,
            target_path,
            status: OperationValidationStatus::Valid,
            reason: None,
            snapshot: Some(snapshot),
            content_fingerprint: item.content_fingerprint().map(str::to_string),
            will_create_directory,
        },
        Err(reason) => OperationPreviewItem {
            index,
            operation,
            source_path,
            target_path,
            status: OperationValidationStatus::Invalid,
            reason: Some(reason),
            snapshot: None,
            content_fingerprint: item.content_fingerprint().map(str::to_string),
            will_create_directory,
        },
    }
}

fn validate_item_inner(
    root: &Path,
    item: &OperationDraftItem,
    source_path: &Path,
    target_path: &Path,
) -> Result<crate::models::operation::FileSnapshot, String> {
    if !is_within(root, source_path) {
        return Err("源文件不在当前授权目录内。".into());
    }
    if has_link_component(root, source_path)? {
        return Err("不支持通过符号链接或重解析点操作文件。".into());
    }
    if has_link_component(root, Path::new(item.source_path()))? {
        return Err("不支持通过符号链接或重解析点操作文件。".into());
    }
    if fs::symlink_metadata(item.source_path())
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("不支持操作符号链接或重解析点。".into());
    }

    let snapshot = snapshot_file(source_path)?;
    if let Some(expected) = item.content_fingerprint() {
        let actual = super::content_extractor::fingerprint_file(source_path)?;
        if actual != expected {
            return Err("AI 建议的内容指纹已失效，请重新分析文件。".into());
        }
    }
    let target_directory = match item {
        OperationDraftItem::Move {
            destination_directory,
            ..
        } => PathBuf::from(destination_directory),
        OperationDraftItem::AiOrganize { category_id, .. } => {
            let directory = category_directory(root, category_id)?;
            if fs::symlink_metadata(&directory).is_ok()
                && !fs::symlink_metadata(&directory)
                    .map_err(|error| error.to_string())?
                    .file_type()
                    .is_dir()
            {
                return Err("分类目标路径已存在但不是目录。".into());
            }
            directory
        }
        OperationDraftItem::Rename { .. } | OperationDraftItem::AiRename { .. } => source_path
            .parent()
            .ok_or_else(|| "源文件父目录不可用。".to_string())?
            .to_path_buf(),
    };
    let target_directory_exists = fs::symlink_metadata(&target_directory).is_ok();
    let normalized_target_directory =
        if matches!(item, OperationDraftItem::AiOrganize { .. }) && !target_directory_exists {
            root.to_path_buf()
        } else {
            normalize_directory(&target_directory)
                .map_err(|_| "目标目录不存在或不是普通目录。".to_string())?
        };
    if has_link_component(root, &target_directory)? {
        return Err("不支持通过符号链接或重解析点写入目标目录。".into());
    }
    if !is_within(root, &normalized_target_directory) {
        return Err("目标目录不在当前授权目录内。".into());
    }
    if has_link_component(root, &normalized_target_directory)? {
        return Err("不支持通过符号链接或重解析点写入目标目录。".into());
    }
    match item {
        OperationDraftItem::Rename { new_name, .. }
        | OperationDraftItem::AiRename { new_name, .. }
        | OperationDraftItem::AiOrganize { new_name, .. } => validate_name(new_name)?,
        OperationDraftItem::Move { .. } => {}
    }
    if !is_within(root, target_path) {
        return Err("目标路径不在当前授权目录内。".into());
    }
    if target_path == source_path {
        return Err("目标路径与源路径相同。".into());
    }
    if fs::symlink_metadata(target_path).is_ok() {
        return Err("目标路径已存在，不能覆盖。".into());
    }
    let target_snapshot = snapshot_directory(&normalized_target_directory)?;
    match (&snapshot.volume_id, &target_snapshot.volume_id) {
        (Some(source_volume), Some(target_volume)) if source_volume == target_volume => {}
        (Some(_), Some(_)) => return Err("不支持跨磁盘或跨文件系统移动。".into()),
        _ => return Err("无法确认源文件和目标目录是否位于同一卷。".into()),
    }
    Ok(snapshot)
}

fn normalize_directory(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir() {
        return Err("路径不是普通目录。".into());
    }
    let canonical = fs::canonicalize(path).map_err(|error| error.to_string())?;
    Ok(canonical)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
    {
        return Err("文件名不合法。".into());
    }
    #[cfg(windows)]
    {
        if name.ends_with('.')
            || name.ends_with(' ')
            || name.chars().any(|character| "<>:\"|?*".contains(character))
            || ["CON", "PRN", "AUX", "NUL"]
                .iter()
                .any(|reserved| name.eq_ignore_ascii_case(reserved))
        {
            return Err("文件名不符合 Windows 文件系统规则。".into());
        }
    }
    Ok(())
}

fn has_link_component(root: &Path, path: &Path) -> Result<bool, String> {
    let relative = match path.strip_prefix(root) {
        Ok(relative) => relative,
        Err(_) => return Ok(false),
    };
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if matches!(component, Component::CurDir | Component::ParentDir) {
            continue;
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(false)
}

fn is_within(root: &Path, path: &Path) -> bool {
    let root = comparable_path(root);
    let path = comparable_path(path);
    path == root || path.strip_prefix(root).is_ok()
}

fn comparable_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        return PathBuf::from(path.to_string_lossy().to_lowercase());
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}
