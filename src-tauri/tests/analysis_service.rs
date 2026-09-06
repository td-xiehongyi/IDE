use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ai_file_organizer_lib::ai::{AiProvider, ProviderAnalysisRequest, ProviderStatus};
use ai_file_organizer_lib::models::ai::{AiSuggestionPayload, Category};
use ai_file_organizer_lib::services::analysis_service::analyze_batch;
use ai_file_organizer_lib::storage::{ai_repository, database};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ai-file-organizer-analysis-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

struct FakeProvider {
    requests: Mutex<Vec<ProviderAnalysisRequest>>,
    response: AiSuggestionPayload,
    mutate_path: Option<std::path::PathBuf>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl FakeProvider {
    fn valid(mutate_path: Option<std::path::PathBuf>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            response: AiSuggestionPayload {
                summary: "摘要".into(),
                keywords: vec!["关键词".into()],
                suggested_filename: "整理后.md".into(),
                category_id: Some("work".into()),
                confidence: 0.9,
                reason: "工作资料".into(),
            },
            mutate_path,
            cancel_flag: None,
        }
    }
}

impl AiProvider for FakeProvider {
    fn provider_id(&self) -> &'static str {
        "fake"
    }
    fn model(&self) -> &str {
        "fake-model"
    }
    fn health(&self) -> Result<ProviderStatus, String> {
        unreachable!()
    }
    fn analyze(&self, request: &ProviderAnalysisRequest) -> Result<AiSuggestionPayload, String> {
        self.requests.lock().unwrap().push(request.clone());
        if let Some(path) = &self.mutate_path {
            fs::write(path, "分析期间被替换").unwrap();
        }
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }
        Ok(self.response.clone())
    }
}

fn categories(root: &std::path::Path) -> Vec<Category> {
    let directory = root.join("work");
    fs::create_dir_all(&directory).unwrap();
    vec![Category {
        id: "work".into(),
        name: "工作".into(),
        description: "工作资料".into(),
        directory_path: directory.to_string_lossy().into(),
        enabled: true,
    }]
}

#[test]
fn analyzes_short_document_and_persists_only_validated_derived_data() {
    let root = temp_dir("short");
    let source = root.join("notes.md");
    fs::write(&source, "项目会议纪要").unwrap();
    let provider = FakeProvider::valid(None);
    let connection = database::open_memory_database().unwrap();

    let outcome = analyze_batch(
        &connection,
        "batch-1",
        &root,
        &[source.to_string_lossy().into()],
        &categories(&root),
        Some(("default", 2)),
        &provider,
        || false,
        |_, _, _| {},
    )
    .unwrap();

    assert!(outcome.failures.is_empty());
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].template_id.as_deref(), Some("default"));
    assert_eq!(outcome.records[0].template_version, Some(2));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        provider.requests.lock().unwrap()[0].language.as_deref(),
        Some("Markdown")
    );
    assert_eq!(
        ai_repository::read_batch_results(&connection, "batch-1")
            .unwrap()
            .len(),
        1
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn long_document_is_analyzed_by_chunks_then_summarized_once() {
    let root = temp_dir("long");
    let source = root.join("long.md");
    fs::write(&source, "文".repeat(8_100)).unwrap();
    let provider = FakeProvider::valid(None);
    let connection = database::open_memory_database().unwrap();

    let outcome = analyze_batch(
        &connection,
        "batch-long",
        &root,
        &[source.to_string_lossy().into()],
        &categories(&root),
        None,
        &provider,
        || false,
        |_, _, _| {},
    )
    .unwrap();

    assert!(outcome.failures.is_empty());
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests.last().unwrap().text.contains("分段分析结果"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discards_result_when_file_changes_during_analysis() {
    let root = temp_dir("changed");
    let source = root.join("notes.md");
    fs::write(&source, "原始正文").unwrap();
    let provider = FakeProvider::valid(Some(source.clone()));
    let connection = database::open_memory_database().unwrap();

    let outcome = analyze_batch(
        &connection,
        "batch-changed",
        &root,
        &[source.to_string_lossy().into()],
        &categories(&root),
        None,
        &provider,
        || false,
        |_, _, _| {},
    )
    .unwrap();

    assert!(outcome.records.is_empty());
    assert_eq!(outcome.failures.len(), 1);
    assert!(outcome.failures[0].reason.contains("发生变化"));
    assert!(
        ai_repository::read_batch_results(&connection, "batch-changed")
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_stops_before_reading_or_calling_provider() {
    let root = temp_dir("cancel");
    let source = root.join("notes.md");
    fs::write(&source, "正文").unwrap();
    let provider = FakeProvider::valid(None);
    let connection = database::open_memory_database().unwrap();

    let error = analyze_batch(
        &connection,
        "batch-cancel",
        &root,
        &[source.to_string_lossy().into()],
        &categories(&root),
        None,
        &provider,
        || true,
        |_, _, _| {},
    )
    .unwrap_err();

    assert!(error.contains("取消"));
    assert!(provider.requests.lock().unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_during_provider_call_does_not_persist_a_result() {
    let root = temp_dir("cancel-during-provider");
    let source = root.join("notes.md");
    fs::write(&source, "正文").unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut provider = FakeProvider::valid(None);
    provider.cancel_flag = Some(cancelled.clone());
    let connection = database::open_memory_database().unwrap();

    let error = analyze_batch(
        &connection,
        "batch-cancel-provider",
        &root,
        &[source.to_string_lossy().into()],
        &categories(&root),
        None,
        &provider,
        || cancelled.load(Ordering::SeqCst),
        |_, _, _| {},
    )
    .unwrap_err();

    assert!(error.contains("取消"));
    assert!(
        ai_repository::read_batch_results(&connection, "batch-cancel-provider")
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}
