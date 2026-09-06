import type { ScanSummary as ScanSummaryData } from "../../types/files";

export function ScanSummary({ summary }: { summary: ScanSummaryData }) {
  const items = [
    ["文件", summary.indexed_files],
    ["目录", summary.indexed_directories],
    ["链接", summary.indexed_links],
    ["新增", summary.added],
    ["更新", summary.updated],
    ["移除", summary.removed],
    ["忽略", summary.ignored],
    ["错误", summary.errors],
  ] as const;
  return (
    <section aria-label="扫描结果摘要" className="rounded-2xl border border-white/10 bg-white/[0.045] p-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-emerald-300">Index Ready</p>
          <h2 className="mt-2 text-2xl font-semibold text-white">索引已更新</h2>
        </div>
        <span className="text-xs text-slate-500">模式：{summary.mode === "rebuild" ? "重建" : "增量扫描"}</span>
      </div>
      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-8">
        {items.map(([label, value]) => <div className="rounded-xl border border-white/10 bg-slate-950/40 p-3" key={label}><div className="text-xs text-slate-500">{label}</div><div className="mt-1 text-xl font-semibold tabular-nums text-white">{value}</div></div>)}
      </div>
    </section>
  );
}
