export function DirectoryPicker({ onChoose, disabled }: { onChoose: () => void; disabled?: boolean }) {
  return <button type="button" disabled={disabled} onClick={onChoose} className="rounded-xl bg-emerald-300 px-5 py-3 text-sm font-semibold text-slate-950 transition hover:bg-emerald-200 disabled:cursor-not-allowed disabled:opacity-50">选择扫描目录</button>;
}
