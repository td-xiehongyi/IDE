import { FileFilters } from "./FileFilters";
import { FileList } from "./FileList";
import { SearchBar } from "./SearchBar";
import type { ReturnTypeUseFiles } from "./useFiles";

export type FileBrowserState = ReturnTypeUseFiles;

export function FileBrowser({ state, changeNotice, watcherError, selectedPaths = new Set(), onToggleSelection = () => undefined }: { state: FileBrowserState; changeNotice: boolean; watcherError: string | null; selectedPaths?: Set<string>; onToggleSelection?: (path: string) => void }) {
  if (!state.result && state.loading) return <section aria-label="文件浏览" className="rounded-2xl border border-white/10 bg-white/[0.045] p-6 text-sm text-slate-400">正在读取文件索引…</section>;
  return (
    <section aria-label="文件浏览" className="space-y-4 rounded-2xl border border-white/10 bg-white/[0.045] p-6">
      <div className="flex flex-wrap items-end justify-between gap-3"><div><p className="text-xs font-semibold uppercase tracking-[0.2em] text-emerald-300">Phase 3 · File Browser</p><h2 className="mt-2 text-2xl font-semibold text-white">浏览已索引文件</h2></div>{changeNotice && <span role="status" className="text-xs text-emerald-200">磁盘变化已同步</span>}</div>
      <SearchBar value={state.queryText} onChange={state.setQueryText} />
      <FileFilters filters={state.filters} onChange={state.updateFilters} sortBy={state.sortBy} onSort={state.updateSort} />
      {watcherError && <div role="alert" className="rounded-xl border border-amber-300/20 bg-amber-300/[0.08] p-3 text-sm text-amber-100">监听提示：{watcherError}</div>}
      {state.error && <div role="alert" className="rounded-xl border border-rose-300/20 bg-rose-300/[0.08] p-3 text-sm text-rose-100">{state.error}</div>}
      {state.result && <><div className="flex items-center justify-between text-xs text-slate-500"><span>共 {state.result.total} 个条目</span>{state.loading && <span>更新中…</span>}</div>{state.result.entries.length ? <FileList entries={state.result.entries} selectedPaths={selectedPaths} onToggle={onToggleSelection} /> : <div className="rounded-xl border border-dashed border-white/10 p-8 text-center text-sm text-slate-500">没有匹配的文件</div>}<div className="flex items-center justify-between"><span className="text-xs text-slate-500">第 {state.result.total_pages ? state.page : 0} / {state.result.total_pages} 页</span><div className="flex gap-2"><button type="button" disabled={state.page <= 1 || state.loading} onClick={() => state.setPage((page) => page - 1)} className="rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-300 disabled:opacity-40">上一页</button><button type="button" disabled={!state.result.total_pages || state.page >= state.result.total_pages || state.loading} onClick={() => state.setPage((page) => page + 1)} className="rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-300 disabled:opacity-40">下一页</button></div></div></>}
    </section>
  );
}
