use crate::models::ai::{AiSuggestionPayload, Category};

pub mod ollama;

pub const PROMPT_VERSION: &str = "phase5-v1";

#[derive(Debug, Clone)]
pub struct ProviderAnalysisRequest {
    pub filename: String,
    pub language: Option<String>,
    pub text: String,
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProviderStatus {
    pub available: bool,
    pub provider: String,
    pub model: String,
    pub message: String,
}

pub trait AiProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn model(&self) -> &str;
    fn health(&self) -> Result<ProviderStatus, String>;
    fn analyze(&self, request: &ProviderAnalysisRequest) -> Result<AiSuggestionPayload, String>;
}
