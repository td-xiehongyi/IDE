export type ProviderStatus = {
  available: boolean;
  provider: string;
  model: string;
  message: string;
};

export type AiCategory = {
  id: string;
  name: string;
  description: string;
  directory_path: string;
  enabled: boolean;
};

export type TemplateCategory = {
  id: string;
  name: string;
  description: string;
  default_enabled: boolean;
};

export type AiCategoryTemplate = {
  id: string;
  name: string;
  version: number;
  categories: TemplateCategory[];
};

export type AnalysisResultStatus = "pending" | "accepted" | "rejected" | "expired";
export type AnalysisBatchStatus = "queued" | "running" | "cancelling" | "completed" | "failed" | "cancelled";

export type AiAnalysisResult = {
  id: string;
  batch_id: string;
  root_path: string;
  source_path: string;
  content_fingerprint: string;
  provider: string;
  model: string;
  prompt_version: string;
  template_id: string | null;
  template_version: number | null;
  summary: string;
  keywords: string[];
  suggested_filename: string;
  category_id: string | null;
  confidence: number;
  reason: string;
  status: AnalysisResultStatus;
  created_at: string;
};

export type AnalysisFailure = { source_path: string; reason: string };

export type AnalysisTask = {
  batch_id: string;
  status: AnalysisBatchStatus;
  total_files: number;
  completed_files: number;
  current_path: string | null;
  result_ids: string[];
  failures: AnalysisFailure[];
  error: string | null;
};

export type AnalysisProgress = {
  batch_id: string;
  phase: "analyzing" | "processing" | "cancelling" | "completed" | "failed" | "cancelled";
  completed_files: number;
  total_files: number;
  current_path: string | null;
  error_count: number;
};
