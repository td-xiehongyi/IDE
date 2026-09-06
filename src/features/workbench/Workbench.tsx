import { useEffect, useMemo, useRef, useState } from "react";

import { getAiProviderStatus } from "../../lib/ai-api";
import { chooseDirectory, chooseFile, chooseTargetDirectory, getIndexStatus, listenForScanProgress, rebuildIndex, restoreRecentIndex, scanDirectory } from "../../lib/files-api";
import { cancelOperationPlan, executeOperationPlan, getOperationHistory, previewOperations, undoOperation } from "../../lib/operations-api";
import { listExplorerChildren, listenForIndexChanges, listenForWatcherErrors, resolveIndexedFile } from "../../lib/search-api";
import type { IndexStatus, ScanProgress, ScanSummary } from "../../types/files";
import type { OperationBatchResult, OperationDraft, OperationHistoryItem, OperationPreviewResponse } from "../../types/operations";
import type { ExplorerNode, SearchEntry } from "../../types/search";
import { AiPanel } from "../ai/AiPanel";
import { OperationHistory } from "../operations/OperationHistory";
import { OperationPanel } from "../operations/OperationPanel";
import { OperationPreview } from "../operations/OperationPreview";
import { ScanProgress as ScanProgressView } from "../files/ScanProgress";
import { ScanSummary as ScanSummaryView } from "../files/ScanSummary";
import { useFiles } from "../files/useFiles";
import { closeDocument, emptyDocumentSession, openDocument } from "./document-session";
import { emptyExplorerState, selectExplorerNode, setExplorerChildren, type SelectionMode } from "./explorer-state";
import { useWorkbenchLayout } from "./useWorkbenchLayout";
import type { ActivityMode, AssistantMode } from "./workbench-types";

const asSearchEntry = (node: ExplorerNode): SearchEntry => ({
  id: node.id,
  normalized_path: node.normalized_path,
  name: node.name,
  extension: node.extension,
  kind: node.kind,
  size: node.size,
  modified_ms: node.modified_ms,
});

const asExplorerNode = (entry: SearchEntry): ExplorerNode => ({ ...entry, kind: entry.kind as ExplorerNode["kind"], has_children: false });
const runningInTauri = () => "__TAURI_INTERNALS__" in window;

export function Workbench() {
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [changeNotice, setChangeNotice] = useState(false);
  const [watcherError, setWatcherError] = useState<string | null>(null);
  const [explorer, setExplorer] = useState(emptyExplorerState);
  const [documents, setDocuments] = useState(emptyDocumentSession);
  const [operationPreview, setOperationPreview] = useState<OperationPreviewResponse | null>(null);
  const [operationBusy, setOperationBusy] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationResult, setOperationResult] = useState<OperationBatchResult | null>(null);
  const [history, setHistory] = useState<OperationHistoryItem[]>([]);
  const [assistantMode, setAssistantMode] = useState<AssistantMode>("ask");
  const [activityMode, setActivityMode] = useState<ActivityMode>("index");
  const [fileMenuOpen, setFileMenuOpen] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [providerMessage, setProviderMessage] = useState("正在检查模型…");
  const { assistantOpen, setAssistantOpen, explorerCollapsed, setExplorerCollapsed, assistantCollapsed, setAssistantCollapsed, activityCollapsed, setActivityCollapsed, leftWidth, setLeftWidth, rightWidth, setRightWidth } = useWorkbenchLayout();
  const fileMenuRef = useRef<HTMLDivElement>(null);
  const assistantInputRef = useRef<HTMLTextAreaElement>(null);
  const browserState = useFiles(rootPath);

  const activeTab = documents.tabs.find((tab) => tab.entry.normalized_path === documents.activePath) ?? null;
  const loadedNodes = useMemo(() => Object.values(explorer.childrenByParent).flat(), [explorer.childrenByParent]);
  const nodesByPath = useMemo(() => new Map([...loadedNodes, ...documents.tabs.map((tab) => tab.entry)].map((node) => [node.normalized_path, node])), [documents.tabs, loadedNodes]);
  const selectedEntries = useMemo(() => [...explorer.selectedPaths].map((path) => nodesByPath.get(path)).filter((node): node is ExplorerNode => node?.kind === "file").map(asSearchEntry), [explorer.selectedPaths, nodesByPath]);
  const agentEntries = selectedEntries.length ? selectedEntries : activeTab?.entry.kind === "file" ? [asSearchEntry(activeTab.entry)] : [];

  useEffect(() => {
    if (!runningInTauri()) return;
    let cleanup: (() => void) | undefined;
    void listenForScanProgress(setProgress).then((next) => { cleanup = next; });
    return () => cleanup?.();
  }, []);

  useEffect(() => {
    let cancelled = false;
    void restoreRecentIndex().then((restored) => {
      if (!cancelled && restored) {
        setRootPath(restored.root_path);
        setStatus(restored);
      }
    }).catch((cause) => { if (runningInTauri()) showError(cause); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!runningInTauri()) return;
    let changeCleanup: (() => void) | undefined;
    let errorCleanup: (() => void) | undefined;
    void listenForIndexChanges(() => {
      setChangeNotice(true);
      setDocuments((current) => ({ ...current, tabs: current.tabs.map((tab) => ({ ...tab, stale: true })) }));
      if (rootPath) void loadChildren(rootPath);
      void browserState.reload();
      window.setTimeout(() => setChangeNotice(false), 2500);
    }).then((next) => { changeCleanup = next; });
    void listenForWatcherErrors(setWatcherError).then((next) => { errorCleanup = next; });
    return () => { changeCleanup?.(); errorCleanup?.(); };
  }, [browserState.reload, rootPath]);

  useEffect(() => {
    if (!rootPath) return;
    void loadChildren(rootPath);
    void refreshHistory();
  }, [rootPath]);

  useEffect(() => {
    void getAiProviderStatus("qwen2.5:7b")
      .then((provider) => setProviderMessage(provider.available ? `${provider.provider} · ${provider.model}` : provider.message))
      .catch(() => setProviderMessage("模型状态不可用"));
  }, []);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (fileMenuOpen && !fileMenuRef.current?.contains(event.target as Node)) setFileMenuOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [fileMenuOpen]);

  useEffect(() => {
    if (fileMenuOpen) window.setTimeout(() => fileMenuRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus(), 0);
  }, [fileMenuOpen]);

  function handleFileMenuKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    if (!items.length) return;
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.key === "ArrowDown" ? (index + 1) % items.length : event.key === "ArrowUp" ? (index - 1 + items.length) % items.length : event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : null;
    if (nextIndex !== null) {
      event.preventDefault();
      items[nextIndex]?.focus();
    }
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (event.ctrlKey && key === "p") { event.preventDefault(); setCommandOpen(true); }
      if (event.ctrlKey && event.shiftKey && key === "f") { event.preventDefault(); document.querySelector<HTMLInputElement>("#explorer-filter")?.focus(); }
      if (event.ctrlKey && key === "w" && documents.activePath) { event.preventDefault(); setDocuments((current) => closeDocument(current, documents.activePath!)); }
      if (event.ctrlKey && key === "tab" && documents.tabs.length) {
        event.preventDefault();
        const index = documents.tabs.findIndex((tab) => tab.entry.normalized_path === documents.activePath);
        const next = documents.tabs[(index + 1) % documents.tabs.length];
        setDocuments((current) => ({ ...current, activePath: next.entry.normalized_path }));
      }
      if (event.ctrlKey && event.shiftKey && key === "a") { event.preventDefault(); setAssistantOpen(true); setAssistantCollapsed(false); window.setTimeout(() => assistantInputRef.current?.focus(), 0); }
      if (event.key === "Escape") { setFileMenuOpen(false); setCommandOpen(false); setAssistantOpen(false); }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [documents]);

  function showError(cause: unknown) {
    setError(cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "操作失败，请重试。");
  }

  function resetForRoot(nextRoot: string) {
    setRootPath(nextRoot);
    setExplorer(emptyExplorerState());
    setDocuments(emptyDocumentSession());
    setOperationPreview(null);
    setOperationResult(null);
    setOperationError(null);
  }

  async function scanSelectedDirectory(selected: string, mode: "incremental" | "rebuild" = "incremental") {
    setError(null); setSummary(null); setWatcherError(null); setBusy(true); resetForRoot(selected);
    try {
      const result = await scanDirectory(selected, mode);
      setSummary(result);
      setStatus({ root_path: result.root_path, indexed_entries: result.indexed_files + result.indexed_directories + result.indexed_links, last_scan_at: result.completed_at, state: "ready" });
      resetForRoot(result.root_path);
    } catch (cause) { showError(cause); } finally { setBusy(false); }
  }

  async function chooseAndScan() {
    try { const selected = await chooseDirectory(); if (selected) await scanSelectedDirectory(selected); } catch (cause) { showError(cause); }
  }

  async function chooseAndOpenFile() {
    setFileMenuOpen(false);
    if (!rootPath) { setError("请先打开并扫描一个授权文件夹。"); return; }
    try {
      const filePath = await chooseFile();
      if (!filePath) return;
      const node = await resolveIndexedFile({ root_path: rootPath, file_path: filePath });
      if (!node) {
        setError("只能打开当前授权根目录内已经建立索引的普通文件。");
        return;
      }
      setDocuments((current) => openDocument(current, node, true));
      setExplorer((current) => selectExplorerNode(current, node, "replace", [node]));
    } catch (cause) { showError(cause); }
  }

  async function handleRebuildIndex() {
    setError(null);
    setBusy(true);
    try {
      await rebuildIndex();
      setExplorer(emptyExplorerState());
      setDocuments(emptyDocumentSession());
      setOperationPreview(null);
      if (rootPath) {
        setStatus(await getIndexStatus(rootPath));
        await loadChildren(rootPath);
      }
    } catch (cause) {
      showError(cause);
    } finally {
      setBusy(false);
    }
  }

  async function loadChildren(parentPath: string) {
    if (!rootPath) return;
    try {
      const children = await listExplorerChildren({ root_path: rootPath, parent_path: parentPath === rootPath ? null : parentPath });
      setExplorer((current) => setExplorerChildren(current, parentPath, children));
    } catch (cause) { showError(cause); }
  }

  function visibleNodes(): ExplorerNode[] {
    if (!rootPath) return [];
    const output: ExplorerNode[] = [];
    const visit = (parent: string) => explorer.childrenByParent[parent]?.forEach((node) => { output.push(node); if (explorer.expandedPaths.has(node.normalized_path)) visit(node.normalized_path); });
    visit(rootPath);
    return output;
  }

  function chooseNode(node: ExplorerNode, event: React.MouseEvent, pinned = false) {
    if (node.kind !== "file") return;
    const mode: SelectionMode = event.shiftKey ? "range" : event.ctrlKey || event.metaKey ? "toggle" : "replace";
    setExplorer((current) => selectExplorerNode(current, node, mode, visibleNodes()));
    setDocuments((current) => openDocument(current, node, pinned));
  }

  async function toggleDirectory(node: ExplorerNode) {
    if (node.kind !== "directory") return;
    const expanded = explorer.expandedPaths.has(node.normalized_path);
    setExplorer((current) => {
      const expandedPaths = new Set(current.expandedPaths);
      expanded ? expandedPaths.delete(node.normalized_path) : expandedPaths.add(node.normalized_path);
      return { ...current, expandedPaths };
    });
    if (!expanded && !explorer.childrenByParent[node.normalized_path]) await loadChildren(node.normalized_path);
  }

  async function handlePreview(draft: OperationDraft) {
    setOperationError(null); setOperationResult(null); setOperationBusy(true);
    try { setOperationPreview(await previewOperations(draft)); } catch (cause) { setOperationError(cause instanceof Error ? cause.message : "无法生成操作预览。"); } finally { setOperationBusy(false); }
  }

  async function handleCancelPreview() {
    if (operationPreview?.planId) try { await cancelOperationPlan(operationPreview.planId); } catch (cause) { setOperationError(cause instanceof Error ? cause.message : "无法取消操作计划。"); }
    setOperationPreview(null);
  }

  async function handleExecute(planId: string) {
    setOperationError(null); setOperationBusy(true);
    try {
      setOperationResult(await executeOperationPlan(planId)); setOperationPreview(null); setExplorer((current) => ({ ...current, selectedPaths: new Set() }));
      await Promise.all([browserState.reload(), refreshHistory(), rootPath ? loadChildren(rootPath) : Promise.resolve()]);
    } catch (cause) { setOperationError(cause instanceof Error ? cause.message : "操作执行失败。"); } finally { setOperationBusy(false); }
  }

  async function refreshHistory() {
    try { setHistory(await getOperationHistory()); } catch (cause) { setOperationError(cause instanceof Error ? cause.message : "无法读取操作历史。"); }
  }

  async function handleUndo(historyId: number) {
    setOperationError(null); setOperationBusy(true);
    try { await undoOperation(historyId); await Promise.all([browserState.reload(), refreshHistory(), rootPath ? loadChildren(rootPath) : Promise.resolve()]); }
    catch (cause) { setOperationError(cause instanceof Error ? cause.message : "撤销失败。"); } finally { setOperationBusy(false); }
  }

  function renderTree(parent: string, depth = 1): React.ReactNode {
    const query = explorer.filterText.trim().toLowerCase();
    return explorer.childrenByParent[parent]?.filter((node) => !query || node.name.toLowerCase().includes(query)).map((node) => {
      const expanded = explorer.expandedPaths.has(node.normalized_path);
      const link = node.kind === "symlink" || node.kind === "junction";
      return <div key={node.normalized_path} role="none">
        <button type="button" role="treeitem" aria-expanded={node.kind === "directory" ? expanded : undefined} aria-selected={explorer.selectedPaths.has(node.normalized_path)} className="wb-tree-row" style={{ paddingLeft: `${8 + depth * 14}px` }} onClick={(event) => node.kind === "directory" ? void toggleDirectory(node) : chooseNode(node, event)} onDoubleClick={(event) => chooseNode(node, event, true)}>
          <span aria-hidden="true">{node.kind === "directory" ? expanded ? "⌄" : "›" : link ? "↗" : ""}</span><span aria-hidden="true">{node.kind === "directory" ? "▣" : node.extension?.toUpperCase().slice(0, 3) || "FILE"}</span><span className="wb-tree-name">{node.name}</span>
        </button>
        {expanded && renderTree(node.normalized_path, depth + 1)}
      </div>;
    });
  }

  return <main className={`workbench ${explorerCollapsed ? "is-explorer-collapsed" : ""} ${assistantCollapsed ? "is-assistant-collapsed" : ""} ${assistantOpen ? "is-assistant-open" : ""} ${activityCollapsed ? "is-activity-collapsed" : ""}`} style={{ "--wb-left": `${leftWidth}px`, "--wb-right": `${rightWidth}px` } as React.CSSProperties}>
    <header className="wb-topbar">
      <div className="wb-brand"><span className="wb-logo" aria-hidden="true">AK</span><div className="wb-file-menu" ref={fileMenuRef}><button type="button" aria-label="文件菜单" aria-haspopup="menu" aria-expanded={fileMenuOpen} onClick={() => setFileMenuOpen((open) => !open)} onKeyDown={(event) => { if (event.key === "ArrowDown") { event.preventDefault(); setFileMenuOpen(true); } }}>文件(F)</button>{fileMenuOpen && <div role="menu" aria-label="文件" onKeyDown={handleFileMenuKeyDown}><button role="menuitem" type="button" onClick={() => void chooseAndOpenFile()}>打开文件…</button><button role="menuitem" type="button" onClick={() => { setFileMenuOpen(false); void chooseAndScan(); }}>打开文件夹…</button></div>}</div><span className="wb-app-name">AI Knowledge Workspace</span></div>
      <div className="wb-command-wrap"><input aria-label="全局搜索" placeholder="搜索文件或命令  Ctrl+P" value={browserState.queryText} onFocus={() => setCommandOpen(true)} onChange={(event) => browserState.setQueryText(event.target.value)} />{commandOpen && rootPath && <div className="wb-command" role="dialog" aria-label="快速打开">{browserState.result?.entries.map((entry) => <button type="button" key={entry.normalized_path} onClick={() => { setDocuments((current) => openDocument(current, asExplorerNode(entry), true)); setCommandOpen(false); }}>{entry.name}<small>{entry.normalized_path}</small></button>)}</div>}</div>
      <div className="wb-top-status"><span className="wb-status-dot" /> <span>{providerMessage}</span><button type="button" aria-label="切换活动窗口" onClick={() => setActivityCollapsed((value) => !value)}>ACT</button><button type="button" aria-label="打开 AI 面板" onClick={() => window.innerWidth < 1100 ? setAssistantOpen((value) => !value) : setAssistantCollapsed((value) => !value)}>AI{operationPreview ? " •" : ""}</button></div>
    </header>

    <div className="wb-main">
      <aside className="wb-explorer" aria-label="Knowledge Explorer">
        <div className="wb-panel-head"><div><strong>KNOWLEDGE EXPLORER</strong><button type="button" aria-label="折叠文件浏览器" onClick={() => setExplorerCollapsed((value) => !value)}>‹</button></div><input id="explorer-filter" aria-label="筛选文件树" placeholder="筛选文件  Ctrl+Shift+F" value={explorer.filterText} onChange={(event) => setExplorer((current) => ({ ...current, filterText: event.target.value }))} /></div>
        <div className="wb-tree" role="tree" aria-label="已授权目录">
          {rootPath ? <><button type="button" role="treeitem" aria-expanded="true" className="wb-tree-row wb-root-row" onClick={() => void loadChildren(rootPath)}><span>⌄</span><span>▣</span><span className="wb-tree-name">{rootPath.split(/[\\/]/).at(-1) || rootPath}</span><span>{status?.indexed_entries ?? 0}</span></button>{renderTree(rootPath)}</> : <div className="wb-empty"><p>尚未打开授权文件夹</p><button type="button" onClick={() => void chooseAndScan()}>选择扫描目录</button></div>}
        </div>
      </aside>
      <PanelResizer side="left" value={leftWidth} setValue={setLeftWidth} min={220} max={480} />

      <section className="wb-documents" aria-label="文档工作区">
        <div className="wb-tabs" role="tablist" aria-label="打开的文档">{documents.tabs.map((tab) => <div key={tab.entry.normalized_path} className={documents.activePath === tab.entry.normalized_path ? "is-active" : ""}><button type="button" role="tab" aria-selected={documents.activePath === tab.entry.normalized_path} onClick={() => setDocuments((current) => ({ ...current, activePath: tab.entry.normalized_path }))}>{tab.entry.name}{tab.stale ? " · 已变化" : tab.pinned ? "" : " · preview"}</button><button type="button" aria-label={`关闭 ${tab.entry.name}`} onClick={() => setDocuments((current) => closeDocument(current, tab.entry.normalized_path))}>×</button></div>)}</div>
        <div className="wb-viewer">{activeTab ? <article className="wb-document-card"><header><h1>{activeTab.entry.name}</h1><p>{activeTab.entry.normalized_path}</p>{activeTab.stale && <div role="status" className="wb-stale">磁盘索引已变化，请重新打开文件以刷新元数据。</div>}</header><dl><div><dt>类型</dt><dd>{activeTab.entry.extension?.toUpperCase() || activeTab.entry.kind}</dd></div><div><dt>大小</dt><dd>{activeTab.entry.size.toLocaleString()} bytes</dd></div><div><dt>修改时间</dt><dd>{activeTab.entry.modified_ms ? new Date(activeTab.entry.modified_ms).toLocaleString() : "未知"}</dd></div></dl><div className="wb-disabled"><strong>正文查看尚未启用</strong><p>阶段六只展示真实索引元数据，不读取或伪造文档正文。</p></div>{operationPreview && <OperationPreview preview={operationPreview} onConfirm={(planId) => void handleExecute(planId)} onCancel={() => void handleCancelPreview()} busy={operationBusy} />}{operationError && <div role="alert" className="wb-alert">{operationError}</div>}{operationResult && <div role="status" className="wb-success">批次完成：{operationResult.items.filter((item) => item.status === "succeeded").length} 项成功。</div>}</article> : <div className="wb-empty"><strong>打开一个文件</strong><p>从左侧已授权目录选择文件，或使用“文件 → 打开文件”。</p>{operationPreview && <OperationPreview preview={operationPreview} onConfirm={(planId) => void handleExecute(planId)} onCancel={() => void handleCancelPreview()} busy={operationBusy} />}</div>}</div>
      </section>

      <PanelResizer side="right" value={rightWidth} setValue={setRightWidth} min={320} max={560} />
      <aside className="wb-assistant" aria-label="AI Assistant">
        <div className="wb-assistant-tabs" role="tablist" aria-label="AI 模式"><button type="button" role="tab" aria-selected={assistantMode === "ask"} onClick={() => setAssistantMode("ask")}>Ask</button><button type="button" role="tab" aria-selected={assistantMode === "agent"} onClick={() => setAssistantMode("agent")}>Agent</button></div>
        <div className="wb-context"><span>Context:</span><button type="button" aria-pressed={!selectedEntries.length}>Current</button><button type="button" aria-pressed={selectedEntries.length > 0}>Selected ({selectedEntries.length})</button><button type="button" disabled>Space</button></div>
        <div className="wb-assistant-content">{assistantMode === "ask" ? <div className="wb-disabled"><strong>Ask 尚未启用</strong><p>阶段八接入带引用的知识检索后开放。</p></div> : rootPath ? <div className="wb-agent-content"><p className="wb-agent-boundary">Agent 只生成整理建议和操作草案；执行仍需预览与确认。</p><AiPanel rootPath={rootPath} selectedEntries={agentEntries} onPreview={handlePreview} onChooseDirectory={chooseTargetDirectory} /><OperationPanel rootPath={rootPath} selectedEntries={agentEntries} onPreview={handlePreview} busy={operationBusy} onChooseTargetDirectory={chooseTargetDirectory} /></div> : <div className="wb-disabled"><strong>尚未授权目录</strong><p>打开并扫描文件夹后才能使用 Agent。</p></div>}</div>
        <div className="wb-compose"><textarea ref={assistantInputRef} aria-label="AI 输入" placeholder={assistantMode === "ask" ? "Ask 尚未启用" : "Agent 使用上方受控操作"} disabled /></div>
      </aside>
    </div>

    <section className="wb-activity" aria-label="活动窗口">
      <div className="wb-activity-tabs" role="tablist" aria-label="活动视图">{(["index", "watcher", "model", "tasks", "history"] as ActivityMode[]).map((mode) => <button type="button" role="tab" aria-selected={activityMode === mode} key={mode} onClick={() => setActivityMode(mode)}>{{ index: "索引", watcher: "监听", model: "模型", tasks: "任务", history: "历史" }[mode]}</button>)}<button type="button" aria-label="收起活动窗口" onClick={() => setActivityCollapsed(true)}>⌄</button></div>
      <div className="wb-activity-content">{activityMode === "index" && <><div className="wb-activity-actions"><button type="button" disabled={!rootPath || busy} onClick={() => rootPath && void scanSelectedDirectory(rootPath)}>重新扫描</button><button type="button" disabled={busy} onClick={() => void handleRebuildIndex()}>重建索引</button><span>{status?.state === "ready" ? `已索引 ${status.indexed_entries} 个条目` : "等待扫描"}</span></div><ScanProgressView progress={progress} />{summary && <ScanSummaryView summary={summary} />}{error && <div role="alert" className="wb-alert">{error}</div>}</>}{activityMode === "watcher" && <div className="wb-log"><span>监听状态</span><span>{watcherError ?? (changeNotice ? "磁盘变化已同步" : rootPath ? "正在监听当前授权根" : "尚未启动")}</span></div>}{activityMode === "model" && <div className="wb-log"><span>Ollama</span><span>{providerMessage}</span></div>}{activityMode === "tasks" && <div className="wb-log"><span>后台任务</span><span>{progress?.phase === "scanning" || progress?.phase === "persisting" ? progress.phase : "没有运行中的扫描任务；Agent 任务状态显示在右侧"}</span></div>}{activityMode === "history" && <OperationHistory items={history} onUndo={(id) => void handleUndo(id)} busy={operationBusy} />}</div>
    </section>
    <footer className="wb-statusbar"><span>✓ 本地优先</span><span>{rootPath ? `当前根：${rootPath}` : "未授权目录"}</span><span>阶段六工作台开发版 · 写操作必须预览并确认</span></footer>
  </main>;
}

function PanelResizer({ side, value, setValue, min, max }: { side: "left" | "right"; value: number; setValue: (value: number) => void; min: number; max: number }) {
  return <div className="wb-resizer" role="separator" aria-label={`${side === "left" ? "Explorer" : "Assistant"} 宽度`} aria-orientation="vertical" aria-valuemin={min} aria-valuemax={max} aria-valuenow={value} onPointerDown={(event) => {
    const start = event.clientX; const initial = value; const target = event.currentTarget; target.setPointerCapture(event.pointerId);
    const move = (next: PointerEvent) => setValue(Math.max(min, Math.min(max, side === "left" ? initial + next.clientX - start : initial - next.clientX + start)));
    const up = () => { target.removeEventListener("pointermove", move); target.removeEventListener("pointerup", up); };
    target.addEventListener("pointermove", move); target.addEventListener("pointerup", up);
  }} />;
}
