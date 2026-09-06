import type { OperationHistoryItem } from "../../types/operations";

export function OperationHistory({
  items,
  onUndo,
  busy,
}: {
  items: OperationHistoryItem[];
  onUndo: (historyId: number) => void;
  busy: boolean;
}) {
  return (
    <section aria-label="操作历史" className="space-y-4 rounded-2xl border border-white/10 bg-white/[0.045] p-5">
      <div><p className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">Operation History</p><h3 className="mt-2 text-lg font-semibold text-white">操作历史与撤销</h3></div>
      {!items.length && <p className="text-sm text-slate-500">还没有实际执行记录。</p>}
      <div className="space-y-2">
        {items.map((item) => (
          <div key={item.id} className="rounded-xl border border-white/10 bg-slate-950/30 p-3 text-sm">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-medium text-white">{item.operation === "move" ? "移动" : "重命名"} · {item.status === "succeeded" ? "成功" : item.status === "failed" ? "失败" : "未执行"}</span>
              {item.undoStatus === "available" && <button type="button" disabled={busy} onClick={() => onUndo(item.id)} className="rounded-lg border border-emerald-300/30 px-3 py-1.5 text-xs text-emerald-200 disabled:opacity-40">撤销</button>}
              {item.undoStatus === "undone" && <span className="text-xs text-slate-500">已撤销</span>}
            </div>
            <div className="mt-2 grid gap-1 text-xs text-slate-400"><span>From：{item.sourcePath}</span><span>To：{item.targetPath}</span></div>
            {item.reason && <p className="mt-2 text-xs text-amber-200">{item.reason}</p>}
            {item.undoStatus === "unavailable" && item.undoReason && <p className="mt-2 text-xs text-slate-500">撤销不可用：{item.undoReason}</p>}
          </div>
        ))}
      </div>
    </section>
  );
}
