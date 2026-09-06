#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AiSuggestionPayload {
    pub summary: String,
    pub keywords: Vec<String>,
    pub suggested_filename: String,
    pub category_id: Option<String>,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
    pub directory_path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TemplateCategory {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CategoryTemplate {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub categories: Vec<TemplateCategory>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ValidatedSuggestion {
    pub summary: String,
    pub keywords: Vec<String>,
    pub suggested_filename: String,
    pub category_id: Option<String>,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisResultStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

impl AnalysisResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("未知 AI 分析结果状态：{value}")),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AiAnalysisRecord {
    pub id: String,
    pub batch_id: String,
    pub root_path: String,
    pub source_path: String,
    pub content_fingerprint: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub template_id: Option<String>,
    pub template_version: Option<i64>,
    pub summary: String,
    pub keywords: Vec<String>,
    pub suggested_filename: String,
    pub category_id: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub status: AnalysisResultStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisBatchStatus {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AnalysisFailure {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AnalysisTaskSnapshot {
    pub batch_id: String,
    pub status: AnalysisBatchStatus,
    pub total_files: usize,
    pub completed_files: usize,
    pub current_path: Option<String>,
    pub result_ids: Vec<String>,
    pub failures: Vec<AnalysisFailure>,
    pub error: Option<String>,
}
