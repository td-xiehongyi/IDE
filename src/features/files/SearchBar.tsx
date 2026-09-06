export function SearchBar({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <label className="block">
      <span className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">搜索名称或路径</span>
      <input aria-label="搜索名称或路径" value={value} onChange={(event) => onChange(event.target.value)} placeholder="例如：报告、项目名或文件夹" className="mt-2 w-full rounded-xl border border-white/10 bg-slate-950/60 px-4 py-3 text-sm text-white outline-none placeholder:text-slate-600 focus:border-emerald-300/50" />
    </label>
  );
}
