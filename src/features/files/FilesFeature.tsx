import { useEffect, useMemo, useState } from "react";

import { chooseDirectory, chooseTargetDirectory, getIndexStatus, listenForScanProgress, rebuildIndex, restoreRecentIndex, scanDirectory } from "../../lib/files-api";
import { cancelOperationPlan, executeOperationPlan, getOperationHistory, previewOperations, undoOperation } from "../../lib/operations-api";
import { listenForIndexChanges, listenForWatcherErrors } from "../../lib/search-api";
import type { IndexStatus, ScanProgress, ScanSummary } from "../../types/files";
import type { OperationBatchResult, OperationDraft, OperationHistoryItem, OperationPreviewResponse } from "../../types/operations";
import { DirectoryPicker } from "./DirectoryPicker";
import { FileBrowser } from "./FileBrowser";
import { ScanProgress as ScanProgressView } from "./ScanProgress";
import { ScanSummary as ScanSummaryView } from "./ScanSummary";
import { useFiles } from "./useFiles";
import { OperationHistory } from "../operations/OperationHistory";
import { OperationPanel } from "../operations/OperationPanel";
import { OperationPreview } from "../operations/OperationPreview";
import { AiPanel } from "../ai/AiPanel";

export function FilesFeature() {
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [changeNotice, setChangeNotice] = useState(false);
  const [watcherError, setWatcherError] = useState<string | null>(null);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [operationPreview, setOperationPreview] = useState<OperationPreviewResponse | null>(null);
  const [operationBusy, setOperationBusy] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationResult, setOperationResult] = useState<OperationBatchResult | null>(null);
  const [history, setHistory] = useState<OperationHistoryItem[]>([]);
  const browserState = useFiles(rootPath);
  const selectedEntries = useMemo(
    () => browserState.result?.entries.filter((entry) => selectedPaths.has(entry.normalized_path) && entry.kind === "file") ?? [],
    [browserState.result, selectedPaths],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForScanProgress(setProgress).then((cleanup) => { unlisten = cleanup; });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let cancelled = false;
    void restoreRecentIndex()
      .then((restored) => {
        if (!cancelled && restored) {
          setRootPath(restored.root_path);
          setStatus(restored);
        }
      })
      .catch((cause) => {
        if (!cancelled) showError(cause);
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let unlistenChange: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    void listenForIndexChanges(() => {
      setChangeNotice(true);
      void browserState.reload();
      window.setTimeout(() => setChangeNotice(false), 2500);
    }).then((cleanup) => { unlistenChange = cleanup; });
    void listenForWatcherErrors(setWatcherError).then((cleanup) => { unlistenError = cleanup; });
    return () => { unlistenChange?.(); unlistenError?.(); };
  }, [browserState.reload]);

  useEffect(() => {
    setSelectedPaths(new Set());
    setOperationPreview(null);
    setOperationResult(null);
  }, [rootPath, browserState.page, browserState.queryText, browserState.sortBy, browserState.sortDirection, browserState.filters]);

  useEffect(() => {
    if (!rootPath) {
      setHistory([]);
      return;
    }
    void refreshHistory();
  }, [rootPath]);

  function showError(cause: unknown) {
    setError(cause instanceof Error ? cause.message : "扫描失败，请检查目录权限后重试。");
    setProgress((current) => current ? { ...current, phase: "failed" } : null);
  }

  async function scanSelectedDirectory(selected: string, mode: "incremental" | "rebuild" = "incremental") {
    setError(null);
    setRootPath(selected);
    setSummary(null);
    setWatcherError(null);
    setBusy(true);
    try {
      const result = await scanDirectory(selected, mode);
      setSummary(result);
      setStatus({ root_path: result.root_path, indexed_entries: result.indexed_files + result.indexed_directories + result.indexed_links, last_scan_at: result.completed_at, state: "ready" });
    } catch (cause) {
      showError(cause);
    } finally {
      setBusy(false);
    }
  }

  async function chooseAndScan() {
    setError(null);
    try {
      const selected = await chooseDirectory();
      if (selected) await scanSelectedDirectory(selected);
    } catch (cause) {
      showError(cause);
    }
  }

  async function rescanCurrentDirectory() {
    if (rootPath) await scanSelectedDirectory(rootPath);
  }

  async function refreshStatus() {
    if (!rootPath) return;
    setStatus(await getIndexStatus(rootPath));
  }

  async function rebuildCurrentIndex() {
    setError(null);
    try {
      await rebuildIndex();
      await Promise.all([refreshStatus(), browserState.reload()]);
    } catch (cause) {
      showError(cause);
    }
  }

  function toggleSelection(path: string) {
    setSelectedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handlePreview(draft: OperationDraft) {
    setOperationError(null);
    setOperationResult(null);
    setOperationBusy(true);
    try {
      setOperationPreview(await previewOperations(draft));
    } catch (cause) {
      setOperationError(cause instanceof Error ? cause.message : "无法生成操作预览。");
    } finally {
      setOperationBusy(false);
    }
  }

  async function handleCancelPreview() {
    if (operationPreview?.planId) {
      try {
        await cancelOperationPlan(operationPreview.planId);
      } catch (cause) {
        setOperationError(cause instanceof Error ? cause.message : "无法取消操作计划。");
      }
    }
    setOperationPreview(null);
  }

  async function handleExecute(planId: string) {
    setOperationError(null);
    setOperationBusy(true);
    try {
      setOperationResult(await executeOperationPlan(planId));
      setOperationPreview(null);
      setSelectedPaths(new Set());
      await Promise.all([browserState.reload(), refreshHistory()]);
    } catch (cause) {
      setOperationError(cause instanceof Error ? cause.message : "操作执行失败。");
    } finally {
      setOperationBusy(false);
    }
  }

  async function refreshHistory() {
    try {
      setHistory(await getOperationHistory());
    } catch (cause) {
      setOperationError(cause instanceof Error ? cause.message : "无法读取操作历史。");
    }
  }

  async function handleUndo(historyId: number) {
    setOperationError(null);
    setOperationBusy(true);
    try {
      await undoOperation(historyId);
      await Promise.all([browserState.reload(), refreshHistory()]);
    } catch (cause) {
      setOperationError(cause instanceof Error ? cause.message : "撤销失败。");
    } finally {
      setOperationBusy(false);
    }
  }

  return (
    <section className="relative z-10 mx-auto w-full max-w-6xl px-6 py-10 sm:px-10 lg:px-14">
      <div className="flex flex-col gap-6 border-b border-white/10 pb-8 sm:flex-row sm:items-end sm:justify-between">
        <div><p className="text-sm font-semibold uppercase tracking-[0.24em] text-emerald-300">Phase 5 · Local AI Suggestions</p><h1 className="mt-3 text-4xl font-semibold tracking-[-0.04em] text-white">扫描、理解并安全整理文件</h1><p className="mt-3 max-w-2xl text-slate-400">本地模型只生成建议；移动与重命名仍需查看 From / To 预览并明确确认。</p></div>
        <DirectoryPicker onChoose={() => void chooseAndScan()} disabled={busy} />
      </div>
      <div className="mt-8 grid gap-4 md:grid-cols-[1fr_auto]">
        <div className="rounded-2xl border border-white/10 bg-white/[0.045] p-5"><div className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">当前授权目录</div><div className="mt-3 truncate text-sm text-slate-200">{rootPath ?? "尚未选择目录"}</div></div>
        <div className="rounded-2xl border border-white/10 bg-white/[0.045] p-5"><div className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">索引状态</div><div className="mt-3 text-sm text-emerald-200">{status?.state === "ready" ? `已索引 ${status.indexed_entries} 个条目` : "等待扫描"}</div></div>
      </div>
      <div className="mt-6 space-y-4">
        {error && <div role="alert" className="rounded-2xl border border-rose-300/20 bg-rose-300/[0.08] p-4 text-sm text-rose-100">{error}</div>}
        <ScanProgressView progress={progress} />
        {summary && <ScanSummaryView summary={summary} />}
        {operationError && <div role="alert" className="rounded-2xl border border-rose-300/20 bg-rose-300/[0.08] p-4 text-sm text-rose-100">{operationError}</div>}
        {rootPath && <AiPanel rootPath={rootPath} selectedEntries={selectedEntries} onPreview={handlePreview} onChooseDirectory={chooseTargetDirectory} />}
        {rootPath && <OperationPanel rootPath={rootPath} selectedEntries={selectedEntries} onPreview={handlePreview} busy={operationBusy} onChooseTargetDirectory={chooseTargetDirectory} />}
        {operationPreview && <OperationPreview preview={operationPreview} onConfirm={(planId) => void handleExecute(planId)} onCancel={() => void handleCancelPreview()} busy={operationBusy} />}
        {operationResult && <div role="status" className="rounded-2xl border border-emerald-300/20 bg-emerald-300/[0.06] p-4 text-sm text-emerald-100">批次完成：{operationResult.items.filter((item) => item.status === "succeeded").length} 项成功，{operationResult.items.filter((item) => item.status === "failed").length} 项失败，{operationResult.items.filter((item) => item.status === "not_executed").length} 项未执行。</div>}
        {rootPath && <FileBrowser state={browserState} changeNotice={changeNotice} watcherError={watcherError} selectedPaths={selectedPaths} onToggleSelection={toggleSelection} />}
        {rootPath && <OperationHistory items={history} onUndo={(historyId) => void handleUndo(historyId)} busy={operationBusy} />}
      </div>
      <div className="mt-6 flex flex-wrap gap-3">
        <button type="button" disabled={!rootPath || busy} onClick={() => void rescanCurrentDirectory()} className="rounded-xl border border-white/10 px-4 py-2 text-sm text-slate-300 hover:border-emerald-300/30 disabled:opacity-40">重新扫描</button>
        <button type="button" disabled={busy} onClick={() => void rebuildCurrentIndex()} className="rounded-xl border border-white/10 px-4 py-2 text-sm text-slate-300 hover:border-emerald-300/30 disabled:opacity-40">重建索引</button>
      </div>
    </section>
  );
}
