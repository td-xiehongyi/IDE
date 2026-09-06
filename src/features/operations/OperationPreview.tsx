import type { OperationPreviewResponse } from "../../types/operations";

export function OperationPreview({
  preview,
  onConfirm,
  onCancel,
  busy,
}: {
  preview: OperationPreviewResponse;
  onConfirm: (planId: string) => void;
  onCancel: () => void;
  busy: boolean;
}) {
  return (
    <section aria-label="操作预览" className="space-y-4 rounded-2xl border border-amber-300/20 bg-amber-300/[0.06] p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-amber-200">安全操作预览</p>
          <h3 className="mt-2 text-lg font-semibold text-white">确认 From / To</h3>
        </div>
        {preview.expiresAt && <span className="text-xs text-slate-400">计划有效期 10 分钟</span>}
      </div>
      <div className="space-y-2">
        {preview.items.map((item) => (
          <div key={item.index} className="rounded-xl border border-white/10 bg-slate-950/40 p-3 text-sm">
            <div className="grid gap-2 md:grid-cols-[1fr_auto_1fr] md:items-center">
              <span className="truncate text-slate-200" title={item.sourcePath}>{item.sourcePath}</span>
              <span className="text-center text-amber-200">→</span>
              <span className="truncate text-slate-200" title={item.targetPath}>{item.targetPath}</span>
            </div>
            <div className="mt-2 text-xs">
              {item.status === "valid" ? <span className="text-emerald-200">校验通过</span> : <span className="text-rose-200">{item.reason ?? "校验失败"}</span>}
              {item.willCreateDirectory && <span className="ml-3 text-amber-200">确认执行时将创建目标分类目录</span>}
            </div>
          </div>
        ))}
      </div>
      <div className="flex justify-end gap-2">
        <button type="button" onClick={onCancel} disabled={busy} className="rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-300 disabled:opacity-40">取消</button>
        {preview.canConfirm && preview.planId && <button type="button" onClick={() => onConfirm(preview.planId!)} disabled={busy} className="rounded-lg bg-amber-200 px-3 py-2 text-xs font-semibold text-slate-950 disabled:opacity-40">{busy ? "执行中…" : "确认执行"}</button>}
      </div>
    </section>
  );
}
