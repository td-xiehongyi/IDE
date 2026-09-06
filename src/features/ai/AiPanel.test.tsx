import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

import type { AnalysisProgress } from "../../types/ai";
import type { SearchEntry } from "../../types/search";
import { AiPanel } from "./AiPanel";

const aiApi = vi.hoisted(() => ({
  applyAiCategoryTemplate: vi.fn(),
  cancelAnalysisBatch: vi.fn(),
  deleteAiCategory: vi.fn(),
  deleteAiCategoryTemplate: vi.fn(),
  getAiCategoryTemplates: vi.fn(),
  getAiCategories: vi.fn(),
  getAiProviderStatus: vi.fn(),
  getAnalysisBatch: vi.fn(),
  getAnalysisResults: vi.fn(),
  listenForAnalysisProgress: vi.fn(),
  reviewAnalysisResult: vi.fn(),
  saveAiCategoryTemplate: vi.fn(),
  saveAiCategories: vi.fn(),
  startAnalysisBatch: vi.fn(),
}));

vi.mock("../../lib/ai-api", () => aiApi);

const selectedEntries: SearchEntry[] = [
  { id: 1, normalized_path: "C:/Docs/notes.md", name: "notes.md", extension: "md", kind: "file", size: 10, modified_ms: 1 },
  { id: 2, normalized_path: "C:/Docs/table.xlsx", name: "table.xlsx", extension: "xlsx", kind: "file", size: 10, modified_ms: 1 },
];

let progressListener: ((progress: AnalysisProgress) => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  progressListener = undefined;
  aiApi.getAiProviderStatus.mockResolvedValue({ available: true, provider: "ollama", model: "qwen2.5:7b", message: "模型已就绪" });
  aiApi.getAiCategoryTemplates.mockResolvedValue([{ id: "default", name: "默认模板", version: 1, categories: [{ id: "work", name: "工作", description: "工作资料", default_enabled: true }] }]);
  aiApi.getAiCategories.mockResolvedValue([{ id: "work", name: "工作", description: "工作资料", directory_path: "C:/Docs/work", enabled: true }]);
  aiApi.listenForAnalysisProgress.mockImplementation(async (listener: (progress: AnalysisProgress) => void) => {
    progressListener = listener;
    return () => undefined;
  });
  aiApi.startAnalysisBatch.mockResolvedValue({ batch_id: "analysis-1" });
  aiApi.getAnalysisResults.mockResolvedValue([{ id: "result-1", batch_id: "analysis-1", root_path: "C:/Docs", source_path: "C:/Docs/notes.md", content_fingerprint: "abc", provider: "ollama", model: "qwen2.5:7b", prompt_version: "phase5-v1", summary: "会议纪要", keywords: ["项目", "会议"], suggested_filename: "项目会议.md", category_id: "work", confidence: 0.9, reason: "工作资料", status: "pending", created_at: "1" }]);
  aiApi.reviewAnalysisResult.mockResolvedValue({ root_path: "C:/Docs", items: [{ operation: "ai_organize", source_path: "C:/Docs/notes.md", category_id: "work", new_name: "最终会议.md", content_fingerprint: "abc" }] });
  aiApi.saveAiCategories.mockResolvedValue([]);
  aiApi.cancelAnalysisBatch.mockResolvedValue(undefined);
  aiApi.deleteAiCategory.mockResolvedValue(undefined);
  aiApi.deleteAiCategoryTemplate.mockResolvedValue(undefined);
  aiApi.saveAiCategoryTemplate.mockResolvedValue({ id: "default", name: "模板", version: 1, categories: [] });
  aiApi.applyAiCategoryTemplate.mockResolvedValue([{ id: "work", name: "工作", description: "工作资料", directory_path: "C:/Docs/work", enabled: true }]);
});

it("analyzes only supported selected files and displays completed suggestions", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  expect(await screen.findByText("模型已就绪")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "分析所选文件（1）" }));
  await waitFor(() => expect(aiApi.startAnalysisBatch).toHaveBeenCalledWith({
    root_path: "C:/Docs",
    file_paths: ["C:/Docs/notes.md"],
    model: "qwen2.5:7b",
  }));

  await waitFor(() => expect(progressListener).toBeDefined());
  progressListener?.({ batch_id: "analysis-1", phase: "completed", completed_files: 1, total_files: 1, current_path: null, error_count: 0 });
  expect(await screen.findByText("会议纪要")).toBeInTheDocument();
  expect(screen.getByText("项目 · 会议")).toBeInTheDocument();
});

it("includes common source files in the analyzable selection", async () => {
  const codeEntries: SearchEntry[] = [
    { id: 3, normalized_path: "C:/Docs/main.cpp", name: "main.cpp", extension: "cpp", kind: "file", size: 10, modified_ms: 1 },
    { id: 4, normalized_path: "C:/Docs/Main.java", name: "Main.java", extension: "java", kind: "file", size: 10, modified_ms: 1 },
  ];
  render(<AiPanel rootPath="C:/Docs" selectedEntries={codeEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  expect(await screen.findByText("模型已就绪")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "分析所选文件（2）" })).not.toBeDisabled();
});

it("accepts an edited suggestion as an operation draft for phase four preview", async () => {
  const onPreview = vi.fn();
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={onPreview} onChooseDirectory={vi.fn()} />);
  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "分析所选文件（1）" }));
  await waitFor(() => expect(progressListener).toBeDefined());
  progressListener?.({ batch_id: "analysis-1", phase: "completed", completed_files: 1, total_files: 1, current_path: null, error_count: 0 });
  await screen.findByText("会议纪要");

  fireEvent.change(screen.getByLabelText("notes.md 的建议文件名"), { target: { value: "最终会议.md" } });
  fireEvent.click(screen.getByRole("button", { name: "接受建议" }));

  await waitFor(() => expect(aiApi.reviewAnalysisResult).toHaveBeenCalledWith({
    result_id: "result-1",
    action: "accept",
    suggested_filename: "最终会议.md",
    category_id: "work",
  }));
  expect(onPreview).toHaveBeenCalledWith(await aiApi.reviewAnalysisResult.mock.results[0].value);
});

it("shows cancellation immediately and recovers the final state through polling", async () => {
  let pollCount = 0;
  aiApi.getAnalysisBatch.mockImplementation(async () => {
    pollCount += 1;
    return {
      batch_id: "analysis-1",
      status: pollCount === 1 ? "cancelling" : "cancelled",
      total_files: 1,
      completed_files: 0,
      current_path: "C:/Docs/notes.md",
      result_ids: [],
      failures: [],
      error: null,
    };
  });

  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);
  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "分析所选文件（1）" }));
  await waitFor(() => expect(aiApi.startAnalysisBatch).toHaveBeenCalled());

  fireEvent.click(screen.getByRole("button", { name: "取消分析" }));
  expect(await screen.findByRole("button", { name: "取消中…" })).toBeDisabled();
  expect(aiApi.cancelAnalysisBatch).toHaveBeenCalledWith("analysis-1");

  await waitFor(() => expect(screen.getByText("0/1 · cancelled")).toBeInTheDocument(), { timeout: 1500 });
  expect(screen.getByRole("button", { name: "分析所选文件（1）" })).not.toBeDisabled();
});

it("keeps analysis disabled when the local model is unavailable", async () => {
  aiApi.getAiProviderStatus.mockResolvedValue({ available: false, provider: "ollama", model: "qwen2.5:7b", message: "模型未安装" });
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);
  expect(await screen.findByText("模型未安装")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "分析所选文件（1）" })).toBeDisabled();
  expect(screen.getByText("本地模型不可用：模型未安装。请确认 Ollama 已启动且模型名称正确，然后刷新状态。")).toBeInTheDocument();
});

it("explains that a supported file must be selected before analysis", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={[]} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  expect(screen.getByRole("button", { name: "分析所选文件（0）" })).toBeDisabled();
  expect(screen.getByText("请在文件列表中勾选至少一个 TXT、MD、PDF、DOCX 或常见代码/配置文件。")).toBeInTheDocument();
});

it("opens category configuration from the missing-category guidance", async () => {
  aiApi.getAiCategories.mockResolvedValue([]);
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  expect(screen.getByRole("button", { name: "分析所选文件（1）" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "现在配置分类" }));
  expect(screen.getByRole("button", { name: "保存分类" })).toBeInTheDocument();
});

it("deletes a local category without deleting its directory", async () => {
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.click(screen.getByRole("button", { name: "删除分类 1" }));

  await waitFor(() => expect(aiApi.deleteAiCategory).toHaveBeenCalledWith("C:/Docs", "work"));
  expect(confirm).toHaveBeenCalled();
  confirm.mockRestore();
});

it("applies a global template after binding its categories to local directories", async () => {
  const onChooseDirectory = vi.fn();
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={onChooseDirectory} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.click(screen.getByRole("button", { name: "应用模板" }));

  await waitFor(() => expect(aiApi.applyAiCategoryTemplate).toHaveBeenCalledWith({
    root_path: "C:/Docs",
    template_id: "default",
    categories: [{ id: "work", name: "工作", description: "工作资料", directory_path: "C:/Docs/work", enabled: true }],
  }));
  expect(onChooseDirectory).not.toHaveBeenCalled();
});

it("creates a local category from a tag without choosing an existing directory", async () => {
  const onChooseDirectory = vi.fn();
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={onChooseDirectory} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.click(screen.getByRole("button", { name: "新增分类" }));
  fireEvent.change(screen.getByLabelText("分类 2 ID"), { target: { value: "game" } });

  expect(screen.getByLabelText("分类 2 名称")).toHaveValue("game");
  expect(screen.getByLabelText("分类 2 目录")).toHaveValue("C:/Docs/game");
  expect(onChooseDirectory).not.toHaveBeenCalled();
});

it("syncs a generated category ID from a safe visible name before saving", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.click(screen.getByRole("button", { name: "新增分类" }));
  fireEvent.change(screen.getByLabelText("分类 2 名称"), { target: { value: "study" } });

  expect(screen.getByLabelText("分类 2 目录")).toHaveValue("C:/Docs/study");
  fireEvent.click(screen.getByRole("button", { name: "保存分类" }));

  await waitFor(() => expect(aiApi.saveAiCategories).toHaveBeenCalledWith("C:/Docs", [
    { id: "work", name: "工作", description: "工作资料", directory_path: "C:/Docs/work", enabled: true },
    { id: "study", name: "study", description: "", directory_path: "C:/Docs/study", enabled: true },
  ]));
});

it("applies a placeholder template category to a derived directory without choosing it", async () => {
  aiApi.getAiCategoryTemplates.mockResolvedValue([{
    id: "default",
    name: "默认模板",
    version: 1,
    categories: [
      { id: "game", name: "新分类", description: "游戏资料", default_enabled: true },
      { id: "study", name: "学习资料", description: "学习资料", default_enabled: true },
    ],
  }]);
  const onChooseDirectory = vi.fn();
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={onChooseDirectory} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.click(screen.getByRole("button", { name: "应用模板" }));

  await waitFor(() => expect(aiApi.applyAiCategoryTemplate).toHaveBeenCalledWith({
    root_path: "C:/Docs",
    template_id: "default",
    categories: [
      { id: "game", name: "game", description: "游戏资料", directory_path: "C:/Docs/game", enabled: true },
      { id: "study", name: "学习资料", description: "学习资料", directory_path: "C:/Docs/study", enabled: true },
    ],
  }));
  expect(onChooseDirectory).not.toHaveBeenCalled();
});

it("syncs a generated template category ID when its visible name is a safe tag", async () => {
  aiApi.getAiCategoryTemplates.mockResolvedValue([{
    id: "default",
    name: "默认模板",
    version: 1,
    categories: [
      { id: "category_1", name: "新分类", description: "", default_enabled: true },
      { id: "category_2", name: "新分类", description: "", default_enabled: true },
    ],
  }]);
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.change(screen.getByLabelText("模板分类 2 名称"), { target: { value: "study" } });
  fireEvent.click(screen.getByRole("button", { name: "应用模板" }));

  await waitFor(() => expect(aiApi.applyAiCategoryTemplate).toHaveBeenCalledWith({
    root_path: "C:/Docs",
    template_id: "default",
    categories: [
      { id: "category_1", name: "category_1", description: "", directory_path: "C:/Docs/category_1", enabled: true },
      { id: "study", name: "study", description: "", directory_path: "C:/Docs/study", enabled: true },
    ],
  }));
});

it("edits and deletes a global template without changing local categories", async () => {
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.change(screen.getByLabelText("模板名称"), { target: { value: "学习模板" } });
  fireEvent.click(screen.getByRole("button", { name: "保存模板" }));

  await waitFor(() => expect(aiApi.saveAiCategoryTemplate).toHaveBeenCalledWith({
    id: "default",
    name: "学习模板",
    categories: [{ id: "work", name: "工作", description: "工作资料", default_enabled: true }],
  }));
  fireEvent.click(screen.getByRole("button", { name: "删除模板" }));
  await waitFor(() => expect(aiApi.deleteAiCategoryTemplate).toHaveBeenCalledWith("default"));
  expect(confirm).toHaveBeenCalled();
  confirm.mockRestore();
});

it("shows the local category name first and keeps its internal ID in collapsed advanced settings", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));

  expect(screen.getByLabelText("分类 1 名称")).toHaveValue("工作");
  const advanced = screen.getAllByText("高级设置").at(-1)!;
  expect(advanced.parentElement).not.toHaveAttribute("open");
  fireEvent.click(advanced);
  expect(screen.getByLabelText("分类 1 ID")).toHaveValue("work");
});

it("saves a renamed local category without changing its internal ID", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  fireEvent.change(screen.getByLabelText("分类 1 名称"), { target: { value: "会议资料" } });
  fireEvent.click(screen.getByRole("button", { name: "保存分类" }));

  await waitFor(() => expect(aiApi.saveAiCategories).toHaveBeenCalledWith("C:/Docs", [{
    id: "work",
    name: "会议资料",
    description: "工作资料",
    directory_path: "C:/Docs/work",
    enabled: true,
  }]));
});

it("uses the category name as the primary field in global template editing", async () => {
  render(<AiPanel rootPath="C:/Docs" selectedEntries={selectedEntries} onPreview={vi.fn()} onChooseDirectory={vi.fn()} />);

  await screen.findByText("模型已就绪");
  fireEvent.click(screen.getByRole("button", { name: "配置分类" }));
  expect(screen.getByLabelText("模板分类 1 名称")).toHaveValue("工作");
  const advancedSections = screen.getAllByText("高级设置");
  expect(advancedSections.length).toBeGreaterThanOrEqual(2);
});
