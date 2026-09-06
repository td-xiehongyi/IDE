use std::path::{Path, PathBuf};

pub fn normalize_root(path: &Path) -> Result<PathBuf, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err("选择的路径不是目录".to_string());
    }
    std::fs::canonicalize(path).map_err(|error| error.to_string())
}

pub fn validate_category_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id == "."
        || id == ".."
        || id.ends_with('.')
        || id.ends_with(' ')
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("分类 ID 必须是单层目录名，只能包含字母、数字、连字符和下划线".into());
    }
    let upper = id.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((1..=9)
            .any(|number| upper == format!("COM{number}") || upper == format!("LPT{number}")))
    {
        return Err("分类 ID 不能使用 Windows 保留设备名".into());
    }
    Ok(())
}

pub fn category_directory(root: &Path, category_id: &str) -> Result<PathBuf, String> {
    validate_category_id(category_id)?;
    Ok(root.join(category_id))
}

pub fn category_directory_tag(category_id: &str, display_name: &str) -> Result<String, String> {
    validate_category_id(category_id)?;
    let name = display_name.trim();
    if is_generated_category_id(category_id) && validate_category_id(name).is_ok() {
        return Ok(name.to_string());
    }
    Ok(category_id.to_string())
}

pub fn category_directory_for_category(
    root: &Path,
    category_id: &str,
    display_name: &str,
) -> Result<PathBuf, String> {
    let tag = category_directory_tag(category_id, display_name)?;
    category_directory(root, &tag)
}

fn is_generated_category_id(id: &str) -> bool {
    id.strip_prefix("category_").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

pub fn is_ignored_name(name: &str, is_dir: bool) -> bool {
    if is_dir {
        matches!(name, ".git" | "node_modules" | "target" | "dist")
    } else {
        name.ends_with(".tmp") || name.ends_with(".temp") || name.starts_with("~$")
    }
}

#[cfg(test)]
mod tests {
    use super::{category_directory, category_directory_for_category, validate_category_id};
    use std::path::Path;

    #[test]
    fn category_ids_are_single_safe_directory_components() {
        assert!(validate_category_id("game-data_1").is_ok());
        assert!(validate_category_id("../outside").is_err());
        assert!(validate_category_id("CON").is_err());
        assert_eq!(
            category_directory(Path::new("C:/Docs"), "game").unwrap(),
            Path::new("C:/Docs/game")
        );
        assert_eq!(
            category_directory_for_category(Path::new("C:/Docs"), "category_2", "study").unwrap(),
            Path::new("C:/Docs/study")
        );
    }
}
