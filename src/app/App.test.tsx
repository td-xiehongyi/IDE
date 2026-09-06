import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";

const api = vi.hoisted(() => ({
  chooseDirectory: vi.fn(),
  chooseFile: vi.fn(),
  chooseTargetDirectory: vi.fn(),
  getIndexStatus: vi.fn(),
  listenForScanProgress: vi.fn(),
  rebuildIndex: vi.fn(),
  restoreRecentIndex: vi.fn(),
  scanDirectory: vi.fn(),
}));

const searchApi = vi.hoisted(() => ({
  listExplorerChildren: vi.fn(),
  listenForIndexChanges: vi.fn(),
  listenForWatcherErrors: vi.fn(),
  resolveIndexedFile: vi.fn(),
  searchFiles: vi.fn(),
}));

const operationsApi = vi.hoisted(() => ({
  cancelOperationPlan: vi.fn(),
  executeOperationPlan: vi.fn(),
  getOperationHistory: vi.fn(),
  previewOperations: vi.fn(),
  undoOperation: vi.fn(),
}));

const aiApi = vi.hoisted(() => ({
  getAiCategoryTemplates: vi.fn(),
  cancelAnalysisBatch: vi.fn(),
  getAiCategories: vi.fn(),
  getAiProviderStatus: vi.fn(),
  getAnalysisBatch: vi.fn(),
  getAnalysisResults: vi.fn(),
  listenForAnalysisProgress: vi.fn(),
  reviewAnalysisResult: vi.fn(),
  saveAiCategories: vi.fn(),
  startAnalysisBatch: vi.fn(),
}));

vi.mock("../lib/files-api", () => api);
vi.mock("../lib/search-api", () => searchApi);
vi.mock("../lib/operations-api", () => operationsApi);
vi.mock("../lib/ai-api", () => aiApi);

describe("App", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.listenForScanProgress.mockResolvedValue(() => undefined);
    api.chooseDirectory.mockResolvedValue("C:/Documents");
    api.chooseFile.mockResolvedValue(null);
    api.chooseTargetDirectory.mockResolvedValue("C:/Documents/archive");
    api.scanDirectory.mockResolvedValue({ root_path: "C:/Documents", mode: "incremental", indexed_files: 3, indexed_directories: 1, indexed_links: 1, added: 5, updated: 0, removed: 0, ignored: 2, errors: 0, completed_at: "now" });
    api.rebuildIndex.mockResolvedValue(undefined);
    api.restoreRecentIndex.mockResolvedValue(null);
    api.getIndexStatus.mockResolvedValue({ root_path: "C:/Documents", indexed_entries: 0, last_scan_at: "now", state: "empty" });
    searchApi.listenForIndexChanges.mockResolvedValue(() => undefined);
    searchApi.listenForWatcherErrors.mockResolvedValue(() => undefined);
    searchApi.searchFiles.mockResolvedValue({ entries: [], total: 0, page: 1, page_size: 50, total_pages: 0 });
    searchApi.listExplorerChildren.mockResolvedValue([]);
    searchApi.resolveIndexedFile.mockResolvedValue(null);
    operationsApi.getOperationHistory.mockResolvedValue([]);
    operationsApi.cancelOperationPlan.mockResolvedValue(undefined);
    operationsApi.executeOperationPlan.mockResolvedValue({ batchId: "batch", items: [] });
    operationsApi.previewOperations.mockResolvedValue({ canConfirm: false, planId: null, expiresAt: null, items: [] });
    operationsApi.undoOperation.mockResolvedValue({});
    aiApi.getAiProviderStatus.mockResolvedValue({ available: true, provider: "ollama", model: "qwen2.5:7b", message: "模型已就绪" });
    aiApi.getAiCategories.mockResolvedValue([{ id: "work", name: "工作", description: "", directory_path: "C:/Documents/work", enabled: true }]);
    aiApi.getAiCategoryTemplates.mockResolvedValue([]);
    aiApi.listenForAnalysisProgress.mockResolvedValue(() => undefined);
    aiApi.getAnalysisResults.mockResolvedValue([]);
    aiApi.cancelAnalysisBatch.mockResolvedValue(undefined);
    aiApi.saveAiCategories.mockResolvedValue([]);
  });

  it("renders the phase six workbench regions and two assistant modes", async () => {
    render(<App />);
    expect(screen.getByRole("button", { name: "文件菜单" })).toBeInTheDocument();
    expect(screen.getByRole("tree", { name: "已授权目录" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "文档工作区" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "AI Assistant" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Ask" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Agent" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Organize" })).not.toBeInTheDocument();
    expect(screen.getByText(/阶段六工作台开发版/)).toBeInTheDocument();
    expect(await screen.findByText("ollama · qwen2.5:7b")).toBeInTheDocument();
  });

  it("shows the local AI suggestion panel after a directory is authorized", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "选择扫描目录" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Agent" }));
    expect(await screen.findByRole("heading", { name: "本地内容分析与整理建议" })).toBeInTheDocument();
    expect(await screen.findByText("模型已就绪")).toBeInTheDocument();
  });

  it("restores the most recent persisted index after restart", async () => {
    api.restoreRecentIndex.mockResolvedValue({ root_path: "D:/Archive", indexed_entries: 1, last_scan_at: "20", state: "ready" });
    searchApi.listExplorerChildren.mockResolvedValue([{ id: 1, normalized_path: "D:/Archive/saved.txt", name: "saved.txt", extension: "txt", kind: "file", size: 1, modified_ms: 1, has_children: false }]);
    render(<App />);
    expect(await screen.findByText(/当前根：D:\/Archive/)).toBeInTheDocument();
    expect(await screen.findByText("saved.txt")).toBeInTheDocument();
    expect(screen.getByText("已索引 1 个条目")).toBeInTheDocument();
  });

  it("shows the scan summary after a directory is selected", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "文件菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "打开文件夹…" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "索引已更新" })).toBeInTheDocument());
    expect(screen.getByText(/当前根：C:\/Documents/)).toBeInTheDocument();
    expect(screen.getByText("已索引 5 个条目")).toBeInTheDocument();
    expect(screen.getByText("目录")).toBeInTheDocument();
    expect(screen.getByText("忽略")).toBeInTheDocument();
  });

  it("opens the directory picker again when selecting another directory", async () => {
    api.chooseDirectory.mockResolvedValueOnce("C:/Documents").mockResolvedValueOnce("D:/Archive");
    api.scanDirectory
      .mockResolvedValueOnce({ root_path: "C:/Documents", mode: "incremental", indexed_files: 3, indexed_directories: 1, indexed_links: 1, added: 5, updated: 0, removed: 0, ignored: 2, errors: 0, completed_at: "now" })
      .mockResolvedValueOnce({ root_path: "D:/Archive", mode: "incremental", indexed_files: 1, indexed_directories: 0, indexed_links: 0, added: 1, updated: 0, removed: 0, ignored: 0, errors: 0, completed_at: "later" });

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "文件菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "打开文件夹…" }));
    await waitFor(() => expect(screen.getByText(/当前根：C:\/Documents/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "文件菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "打开文件夹…" }));
    await waitFor(() => expect(screen.getByText(/当前根：D:\/Archive/)).toBeInTheDocument());
    expect(api.chooseDirectory).toHaveBeenCalledTimes(2);
    expect(api.scanDirectory).toHaveBeenCalledTimes(2);
  });

  it("rescans the current directory without reopening the picker", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "选择扫描目录" }));
    await waitFor(() => expect(screen.getByText(/当前根：C:\/Documents/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "重新扫描" }));
    await waitFor(() => expect(api.scanDirectory).toHaveBeenCalledTimes(2));
    expect(api.chooseDirectory).toHaveBeenCalledTimes(1);
  });

  it("shows an error when directory selection fails", async () => {
    api.chooseDirectory.mockRejectedValueOnce(new Error("dialog unavailable"));

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "选择扫描目录" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("dialog unavailable");
  });

  it("clears visible file results after rebuilding the index", async () => {
    searchApi.listExplorerChildren
      .mockResolvedValueOnce([{ id: 1, normalized_path: "C:/Documents/report.txt", name: "report.txt", extension: "txt", kind: "file", size: 10, modified_ms: 1, has_children: false }])
      .mockResolvedValue([]);
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "选择扫描目录" }));
    expect(await screen.findByText("report.txt")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重建索引" }));
    await waitFor(() => expect(screen.queryByText("report.txt")).not.toBeInTheDocument());
    expect(searchApi.listExplorerChildren).toHaveBeenCalledTimes(2);
  });

  it("does not expose operation controls before a directory is selected", () => {
    render(<App />);
    for (const action of ["批量移动", "重命名", "删除", "AI 执行"]) {
      expect(screen.queryByRole("button", { name: action })).not.toBeInTheDocument();
    }
  });

  it("supports keyboard navigation and Escape in the file menu", async () => {
    render(<App />);
    const trigger = screen.getByRole("button", { name: "文件菜单" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const openFile = await screen.findByRole("menuitem", { name: "打开文件…" });
    await waitFor(() => expect(openFile).toHaveFocus());
    fireEvent.keyDown(openFile, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "打开文件夹…" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("opens only a resolved indexed file as a pinned metadata tab", async () => {
    api.restoreRecentIndex.mockResolvedValue({ root_path: "C:/Documents", indexed_entries: 1, last_scan_at: "now", state: "ready" });
    api.chooseFile.mockResolvedValue("C:/Documents/report.md");
    searchApi.resolveIndexedFile.mockResolvedValue({ id: 7, normalized_path: "C:/Documents/report.md", name: "report.md", extension: "md", kind: "file", size: 42, modified_ms: 1, has_children: false });
    render(<App />);
    await screen.findByText(/当前根：C:\/Documents/);
    fireEvent.click(screen.getByRole("button", { name: "文件菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "打开文件…" }));
    expect(await screen.findByRole("tab", { name: "report.md" })).toBeInTheDocument();
    expect(screen.getByText("正文查看尚未启用")).toBeInTheDocument();
  });
});
