import type { SearchSortField } from "../../types/search";
import type { FileFilters as FileFiltersState } from "./useFiles";

type Props = {
  filters: FileFiltersState;
  onChange: (next: Partial<FileFiltersState>) => void;
  sortBy: SearchSortField;
  onSort: (field: SearchSortField) => void;
};

export function FileFilters({ filters, onChange, sortBy, onSort }: Props) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
      <input aria-label="扩展名筛选" value={filters.extension} onChange={(event) => onChange({ extension: event.target.value })} placeholder="扩展名，如 pdf" className="rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2 text-sm text-white placeholder:text-slate-600" />
      <input aria-label="最小大小" inputMode="numeric" value={filters.minSize} onChange={(event) => onChange({ minSize: event.target.value })} placeholder="最小大小（字节）" className="rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2 text-sm text-white placeholder:text-slate-600" />
      <input aria-label="最大大小" inputMode="numeric" value={filters.maxSize} onChange={(event) => onChange({ maxSize: event.target.value })} placeholder="最大大小（字节）" className="rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2 text-sm text-white placeholder:text-slate-600" />
      <input aria-label="修改起始日期" type="date" value={filters.modifiedAfter} onChange={(event) => onChange({ modifiedAfter: event.target.value })} className="rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2 text-sm text-slate-300" />
      <input aria-label="修改结束日期" type="date" value={filters.modifiedBefore} onChange={(event) => onChange({ modifiedBefore: event.target.value })} className="rounded-xl border border-white/10 bg-slate-950/60 px-3 py-2 text-sm text-slate-300" />
      <label className="flex items-center gap-2 text-sm text-slate-400 sm:col-span-2 lg:col-span-5">
        排序：
        <select aria-label="排序字段" value={sortBy} onChange={(event) => onSort(event.target.value as SearchSortField)} className="rounded-lg border border-white/10 bg-slate-950 px-2 py-2 text-slate-200">
          <option value="modified">修改时间</option><option value="name">名称</option><option value="path">路径</option><option value="extension">类型</option><option value="size">大小</option>
        </select>
        <span className="text-xs text-slate-500">再次点击同一字段可切换升降序</span>
      </label>
    </div>
  );
}
