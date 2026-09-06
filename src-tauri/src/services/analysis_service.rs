use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::ai::{AiProvider, PROMPT_VERSION, ProviderAnalysisRequest};
use crate::models::ai::{
    AiAnalysisRecord, AnalysisFailure, AnalysisResultStatus, Category, ValidatedSuggestion,
};
use crate::storage::ai_repository;

use super::content_chunker::{DEFAULT_CHUNK_CHARACTERS, DEFAULT_CHUNK_OVERLAP, chunk_text};
use super::content_extractor::{ExtractionLimits, document_language, extract_document};
use super::suggestion_validator::validate_suggestion;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BatchAnalysisOutcome {
    pub records: Vec<AiAnalysisRecord>,
    pub failures: Vec<AnalysisFailure>,
}

#[allow(clippy::too_many_arguments)]
pub fn analyze_batch(
    connection: &Connection,
    batch_id: &str,
    root: &Path,
    files: &[String],
    categories: &[Category],
    template: Option<(&str, i64)>,
    provider: &dyn AiProvider,
    should_cancel: impl Fn() -> bool,
    on_progress: impl Fn(usize, usize, Option<String>),
) -> Result<BatchAnalysisOutcome, String> {
    let mut records = Vec::new();
    let mut failures = Vec::new();
    for (index, source_path) in files.iter().enumerate() {
        if should_cancel() {
            return Err("分析批次已取消".into());
        }
        on_progress(index, files.len(), Some(source_path.clone()));
        match analyze_one(
            connection,
            batch_id,
            index,
            root,
            Path::new(source_path),
            categories,
            template,
            provider,
            &should_cancel,
        ) {
            Ok(record) => records.push(record),
            Err(reason) => {
                if should_cancel() {
                    return Err("分析批次已取消".into());
                }
                failures.push(AnalysisFailure {
                    source_path: source_path.clone(),
                    reason,
                });
            }
        }
        on_progress(index + 1, files.len(), None);
    }
    Ok(BatchAnalysisOutcome { records, failures })
}

#[allow(clippy::too_many_arguments)]
fn analyze_one(
    connection: &Connection,
    batch_id: &str,
    index: usize,
    root: &Path,
    source: &Path,
    categories: &[Category],
    template: Option<(&str, i64)>,
    provider: &dyn AiProvider,
    should_cancel: &impl Fn() -> bool,
) -> Result<AiAnalysisRecord, String> {
    let extracted = extract_document(root, source, ExtractionLimits::default())?;
    let chunks = chunk_text(
        &extracted.text,
        DEFAULT_CHUNK_CHARACTERS,
        DEFAULT_CHUNK_OVERLAP,
    )?;
    let filename = extracted
        .source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "文件名不是有效的 Unicode 文本".to_string())?
        .to_string();
    let language = document_language(&extracted.source_path).map(str::to_owned);
    let suggestion = if chunks.len() == 1 {
        if should_cancel() {
            return Err("分析批次已取消".into());
        }
        provider.analyze(&ProviderAnalysisRequest {
            filename: filename.clone(),
            language: language.clone(),
            text: chunks[0].clone(),
            categories: categories.to_vec(),
        })?
    } else {
        let mut partials: Vec<ValidatedSuggestion> = Vec::with_capacity(chunks.len());
        for (chunk_index, chunk) in chunks.into_iter().enumerate() {
            if should_cancel() {
                return Err("分析批次已取消".into());
            }
            let payload = provider.analyze(&ProviderAnalysisRequest {
                filename: filename.clone(),
                language: language.clone(),
                text: format!("第 {} 段正文：\n{chunk}", chunk_index + 1),
                categories: categories.to_vec(),
            })?;
            partials.push(validate_suggestion(source, payload, categories)?);
        }
        let partial_json = serde_json::to_string(&partials)
            .map_err(|error| format!("无法汇总分段分析结果：{error}"))?;
        provider.analyze(&ProviderAnalysisRequest {
            filename: filename.clone(),
            language,
            text: format!(
                "请根据以下分段分析结果生成整份文档的最终结果：\n分段分析结果：{partial_json}"
            ),
            categories: categories.to_vec(),
        })?
    };
    if should_cancel() {
        return Err("分析批次已取消".into());
    }
    let suggestion = validate_suggestion(source, suggestion, categories)?;
    let current = extract_document(root, source, ExtractionLimits::default())?;
    if current.content_fingerprint != extracted.content_fingerprint {
        return Err("文件在分析期间发生变化，结果已丢弃".into());
    }
    let record = AiAnalysisRecord {
        id: format!("result-{batch_id}-{index}"),
        batch_id: batch_id.into(),
        root_path: root.to_string_lossy().into(),
        source_path: extracted.source_path.to_string_lossy().into(),
        content_fingerprint: extracted.content_fingerprint,
        provider: provider.provider_id().into(),
        model: provider.model().into(),
        prompt_version: PROMPT_VERSION.into(),
        template_id: template.map(|(id, _)| id.into()),
        template_version: template.map(|(_, version)| version),
        summary: suggestion.summary,
        keywords: suggestion.keywords,
        suggested_filename: suggestion.suggested_filename,
        category_id: suggestion.category_id,
        confidence: suggestion.confidence,
        reason: suggestion.reason,
        status: AnalysisResultStatus::Pending,
        created_at: now_string(),
    };
    ai_repository::insert_analysis_result(connection, &record)
        .map_err(|error| format!("无法保存 AI 分析结果：{error}"))?;
    Ok(record)
}

fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
