use std::path::Path;

use rusqlite::Connection;

use crate::models::ai::{AiSuggestionPayload, AnalysisResultStatus};
use crate::models::operation::{OperationDraft, OperationDraftItem};
use crate::storage::ai_repository;

use super::content_extractor::fingerprint_file;
use super::path_policy::category_directory_for_category;
use super::suggestion_validator::validate_suggestion;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Accept,
    Reject,
}

pub fn review_result(
    connection: &Connection,
    result_id: &str,
    action: ReviewAction,
    suggested_filename: Option<String>,
    category_id: Option<String>,
) -> Result<Option<OperationDraft>, String> {
    let record = ai_repository::read_result(connection, result_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "AI 分析结果不存在".to_string())?;
    if record.status != AnalysisResultStatus::Pending {
        return Err("该 AI 分析结果已经审查或过期".into());
    }
    if action == ReviewAction::Reject {
        ai_repository::update_result_status(connection, result_id, AnalysisResultStatus::Rejected)
            .map_err(|error| error.to_string())?;
        return Ok(None);
    }
    let actual_fingerprint = fingerprint_file(Path::new(&record.source_path))?;
    if actual_fingerprint != record.content_fingerprint {
        ai_repository::update_result_status(connection, result_id, AnalysisResultStatus::Expired)
            .map_err(|error| error.to_string())?;
        return Err("文件内容已变化，AI 分析结果已过期".into());
    }
    let categories = ai_repository::read_categories(connection, &record.root_path)
        .map_err(|error| error.to_string())?;
    let filename = suggested_filename.unwrap_or_else(|| record.suggested_filename.clone());
    let validated = validate_suggestion(
        Path::new(&record.source_path),
        AiSuggestionPayload {
            summary: record.summary.clone(),
            keywords: record.keywords.clone(),
            suggested_filename: filename,
            category_id: category_id.clone(),
            confidence: record.confidence,
            reason: record.reason.clone(),
        },
        &categories,
    )?;
    let source = Path::new(&record.source_path);
    let item = if let Some(category_id) = validated.category_id.as_deref() {
        let category = categories
            .iter()
            .find(|category| category.enabled && category.id == category_id)
            .ok_or_else(|| "所选分类不存在或未启用".to_string())?;
        let root = Path::new(&record.root_path);
        let destination = category_directory_for_category(root, &category.id, &category.name)?;
        if source.parent().is_some_and(|parent| {
            destination.exists()
                && std::fs::canonicalize(parent).ok().as_ref()
                    == std::fs::canonicalize(&destination).ok().as_ref()
        }) {
            OperationDraftItem::AiRename {
                source_path: record.source_path.clone(),
                new_name: validated.suggested_filename,
                content_fingerprint: record.content_fingerprint.clone(),
            }
        } else {
            OperationDraftItem::AiOrganize {
                source_path: record.source_path.clone(),
                category_id: destination
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| "分类目标目录名称不可用".to_string())?
                    .into(),
                new_name: validated.suggested_filename,
                content_fingerprint: record.content_fingerprint.clone(),
            }
        }
    } else {
        OperationDraftItem::AiRename {
            source_path: record.source_path.clone(),
            new_name: validated.suggested_filename,
            content_fingerprint: record.content_fingerprint.clone(),
        }
    };
    ai_repository::update_result_status(connection, result_id, AnalysisResultStatus::Accepted)
        .map_err(|error| error.to_string())?;
    Ok(Some(OperationDraft {
        root_path: record.root_path,
        items: vec![item],
    }))
}
