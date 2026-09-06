import { useEffect, useState } from "react";

import type { SearchEntry } from "../../types/search";
import type { OperationDraft } from "../../types/operations";

export function OperationPanel({
  rootPath,
  selectedEntries,
  onPreview,
  busy,
  onChooseTargetDirectory,
}: {
  rootPath: string;
  selectedEntries: SearchEntry[];
  onPreview: (draft: OperationDraft) => void;
  busy: boolean;
  onChooseTargetDirectory?: () => Promise<string | null>;
}) {
  const [mode, setMode] = useState<"move" | "rename">("move");
  const [destinationDirectory, setDestinationDirectory] = useState("");
  const [newName, setNewName] = useState("");

  useEffect(() => {
    setNewName(selectedEntries[0]?.name ?? "");
  }, [selectedEntries]);

  if (!selectedEntries.length) return null;

  async function chooseTarget() {
    const selected = await onChooseTargetDirectory?.();
    if (selected) setDestinationDirectory(selected);
  }

  function submit() {
    if (mode === "move" && destinationDirectory.trim()) {
      onPreview({
        root_path: rootPath,
        items: selectedEntries.map((entry) => ({
          operation: "move",
          source_path: entry.normalized_path,
          destination_directory: destinationDirectory.trim(),
        })),
      });
    }
    if (mode === "rename" && selectedEntries.length === 1 && newName.trim()) {
      onPreview({
        root_path: rootPath,
        items: [{ operation: "rename", source_path: selectedEntries[0].normalized_path, new_name: newName.trim() }],
      });
    }
  }

  return (
    <section aria-label="文件操作" className="space-y-4 rounded-2xl border border-emerald-300/20 bg-emerald-300/[0.05] p-5">
      <div className="flex flex-wrap items-center justify-between gap-3"><div><p className="text-xs font-semibold uppercase tracking-[0.2em] text-emerald-200">Selected Files</p><h3 className="mt-2 text-lg font-semibold text-white">已选择 {selectedEntries.length} 个普通文件</h3></div><div className="flex gap-2"><button type="button" onClick={() => setMode("move")} className={`rounded-lg px-3 py-2 text-xs ${mode === "move" ? "bg-emerald-200 text-slate-950" : "border border-white/10 text-slate-300"}`}>批量移动</button><button type="button" disabled={selectedEntries.length !== 1} onClick={() => setMode("rename")} className={`rounded-lg px-3 py-2 text-xs disabled:opacity-40 ${mode === "rename" ? "bg-emerald-200 text-slate-950" : "border border-white/10 text-slate-300"}`}>重命名</button></div></div>
      {mode === "move" ? <div className="flex flex-col gap-2 sm:flex-row"><input aria-label="移动到目录" value={destinationDirectory} onChange={(event) => setDestinationDirectory(event.target.value)} placeholder="输入当前授权目录内的目标目录" className="min-w-0 flex-1 rounded-lg border border-white/10 bg-slate-950/50 px-3 py-2 text-sm text-white outline-none placeholder:text-slate-600" />{onChooseTargetDirectory && <button type="button" onClick={() => void chooseTarget()} className="rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-300">选择目录</button>}</div> : <input aria-label="新文件名" value={newName} onChange={(event) => setNewName(event.target.value)} className="w-full rounded-lg border border-white/10 bg-slate-950/50 px-3 py-2 text-sm text-white outline-none" />}
      {mode === "rename" && selectedEntries.length !== 1 && <p className="text-xs text-amber-200">重命名一次只能处理一个文件。</p>}
      <div className="flex justify-end"><button type="button" onClick={submit} disabled={busy || (mode === "move" ? !destinationDirectory.trim() : selectedEntries.length !== 1 || !newName.trim())} className="rounded-lg bg-emerald-200 px-4 py-2 text-xs font-semibold text-slate-950 disabled:opacity-40">生成预览</button></div>
    </section>
  );
}
