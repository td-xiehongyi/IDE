import type { SearchEntry } from "../../types/search";

function formatSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function FileList({ entries, selectedPaths, onToggle }: { entries: SearchEntry[]; selectedPaths: Set<string>; onToggle: (path: string) => void }) {
  return (
    <div className="overflow-x-auto rounded-2xl border border-white/10">
      <table className="w-full min-w-[760px] text-left text-sm">
        <thead className="bg-white/[0.04] text-xs uppercase tracking-[0.14em] text-slate-500"><tr><th className="w-10 px-4 py-3">选择</th><th className="px-4 py-3">名称</th><th className="px-4 py-3">路径</th><th className="px-4 py-3">类型</th><th className="px-4 py-3">大小</th><th className="px-4 py-3">修改时间</th></tr></thead>
        <tbody className="divide-y divide-white/5">{entries.map((entry) => { const selectable = entry.kind === "file"; return <tr key={entry.id} className="text-slate-300"><td className="px-4 py-3"><input aria-label={`选择 ${entry.name}`} type="checkbox" disabled={!selectable} checked={selectable && selectedPaths.has(entry.normalized_path)} onChange={() => onToggle(entry.normalized_path)} /></td><td className="max-w-[220px] truncate px-4 py-3 font-medium text-white">{entry.name}</td><td className="max-w-[300px] truncate px-4 py-3 text-slate-400" title={entry.normalized_path}>{entry.normalized_path}</td><td className="px-4 py-3 text-slate-400">{entry.extension ? `.${entry.extension}` : entry.kind}</td><td className="px-4 py-3 tabular-nums">{formatSize(entry.size)}</td><td className="px-4 py-3 text-slate-400">{entry.modified_ms ? new Date(entry.modified_ms).toLocaleString() : "—"}</td></tr>; })}</tbody>
      </table>
    </div>
  );
}
