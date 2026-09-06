import type { ScanProgress as ScanProgressData } from "../../types/files";

type Props = { progress: ScanProgressData | null };

export function ScanProgress({ progress }: Props) {
  if (!progress) return null;
  const phaseLabel = { scanning: "正在扫描", persisting: "正在保存索引", completed: "扫描完成", failed: "扫描失败" }[progress.phase];
  return (
    <section aria-label="扫描进度" className="rounded-2xl border border-emerald-300/20 bg-emerald-300/[0.06] p-6">
      <div className="flex items-center justify-between gap-4">
        <h2 className="text-lg font-semibold text-white">{phaseLabel}</h2>
        <span className="text-sm tabular-nums text-emerald-200">{progress.visitedEntries} 个条目</span>
      </div>
      <div className="mt-5 h-2 overflow-hidden rounded-full bg-white/10">
        <div className="h-full w-full animate-pulse rounded-full bg-emerald-300/70" />
      </div>
      <div className="mt-4 grid gap-3 text-sm text-slate-300 sm:grid-cols-3">
        <span>已索引 {progress.indexedEntries}</span>
        <span>错误 {progress.errorCount}</span>
        <span className="truncate" title={progress.currentPath ?? undefined}>{progress.currentPath ?? "准备中"}</span>
      </div>
    </section>
  );
}
