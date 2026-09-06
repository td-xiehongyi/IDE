use std::collections::HashSet;
use std::path::Path;

use crate::models::ai::{AiSuggestionPayload, Category, ValidatedSuggestion};

pub fn validate_suggestion(
    source_path: &Path,
    payload: AiSuggestionPayload,
    categories: &[Category],
) -> Result<ValidatedSuggestion, String> {
    if payload.summary.trim().is_empty() || payload.reason.trim().is_empty() {
        return Err("摘要和建议理由不能为空".into());
    }
    if payload.keywords.is_empty() || payload.keywords.iter().any(|value| value.trim().is_empty()) {
        return Err("关键词必须为非空字符串数组".into());
    }
    let mut seen = HashSet::new();
    if payload
        .keywords
        .iter()
        .map(|value| value.trim().to_lowercase())
        .any(|value| !seen.insert(value))
    {
        return Err("关键词不能重复".into());
    }
    if !payload.confidence.is_finite() || !(0.0..=1.0).contains(&payload.confidence) {
        return Err("置信度必须位于 0 到 1 之间".into());
    }
    validate_filename(source_path, &payload.suggested_filename)?;
    if let Some(category_id) = payload.category_id.as_deref()
        && !categories
            .iter()
            .any(|category| category.enabled && category.id == category_id)
    {
        return Err("模型返回了未知或未启用的分类 ID".into());
    }
    Ok(ValidatedSuggestion {
        summary: payload.summary.trim().into(),
        keywords: payload
            .keywords
            .into_iter()
            .map(|value| value.trim().to_string())
            .collect(),
        suggested_filename: payload.suggested_filename,
        category_id: payload.category_id,
        confidence: payload.confidence,
        reason: payload.reason.trim().into(),
    })
}

fn validate_filename(source_path: &Path, filename: &str) -> Result<(), String> {
    let candidate = Path::new(filename);
    if filename.trim().is_empty()
        || candidate.file_name().and_then(|value| value.to_str()) != Some(filename)
        || filename.contains(['/', '\\'])
        || filename == "."
        || filename == ".."
    {
        return Err("建议文件名不能包含路径".into());
    }
    if filename
        .chars()
        .any(|character| character.is_control() || "<>:\"|?*".contains(character))
    {
        return Err("建议文件名包含非法字符".into());
    }
    let source_extension = source_path.extension().and_then(|value| value.to_str());
    let candidate_extension = candidate.extension().and_then(|value| value.to_str());
    if source_extension.map(str::to_lowercase) != candidate_extension.map(str::to_lowercase) {
        return Err("建议文件名必须保留原扩展名".into());
    }
    Ok(())
}
