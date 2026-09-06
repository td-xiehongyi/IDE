import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  AiAnalysisResult,
  AiCategory,
  AiCategoryTemplate,
  AnalysisProgress,
  AnalysisTask,
  ProviderStatus,
} from "../types/ai";
import type { OperationDraft } from "../types/operations";

export function getAiProviderStatus(model?: string): Promise<ProviderStatus> {
  return invoke("get_ai_provider_status", { model });
}

export function getAiCategories(rootPath: string): Promise<AiCategory[]> {
  return invoke("get_ai_categories", { rootPath });
}

export function saveAiCategories(rootPath: string, categories: AiCategory[]): Promise<AiCategory[]> {
  return invoke("save_ai_categories", { rootPath, categories });
}

export function getAiCategoryTemplates(): Promise<AiCategoryTemplate[]> {
  return invoke("get_ai_category_templates");
}

export function saveAiCategoryTemplate(request: {
  id: string;
  name: string;
  categories: AiCategoryTemplate["categories"];
}): Promise<AiCategoryTemplate> {
  return invoke("save_ai_category_template", { request });
}

export function deleteAiCategoryTemplate(templateId: string): Promise<void> {
  return invoke("delete_ai_category_template", { templateId });
}

export function applyAiCategoryTemplate(request: {
  root_path: string;
  template_id: string;
  categories: AiCategory[];
}): Promise<AiCategory[]> {
  return invoke("apply_ai_category_template", { request });
}

export function deleteAiCategory(rootPath: string, categoryId: string): Promise<void> {
  return invoke("delete_ai_category", { rootPath, categoryId });
}

export function startAnalysisBatch(request: {
  root_path: string;
  file_paths: string[];
  model: string;
}): Promise<{ batch_id: string }> {
  return invoke("start_analysis_batch", { request });
}

export function getAnalysisBatch(batchId: string): Promise<AnalysisTask> {
  return invoke("get_analysis_batch", { batchId });
}

export function cancelAnalysisBatch(batchId: string): Promise<void> {
  return invoke("cancel_analysis_batch", { batchId });
}

export function getAnalysisResults(batchId: string): Promise<AiAnalysisResult[]> {
  return invoke("get_analysis_results", { batchId });
}

export function reviewAnalysisResult(request: {
  result_id: string;
  action: "accept" | "reject";
  suggested_filename: string | null;
  category_id: string | null;
}): Promise<OperationDraft | null> {
  return invoke("review_analysis_result", { request });
}

export async function listenForAnalysisProgress(
  listener: (progress: AnalysisProgress) => void,
): Promise<() => void> {
  return listen<AnalysisProgress>("ai://analysis-progress", (event) => listener(event.payload));
}
