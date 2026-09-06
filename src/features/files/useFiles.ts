import { useCallback, useEffect, useMemo, useState } from "react";

import { searchFiles } from "../../lib/search-api";
import type { SearchQuery, SearchResult, SearchSortDirection, SearchSortField } from "../../types/search";

const PAGE_SIZE = 50;

export type FileFilters = {
  extension: string;
  minSize: string;
  maxSize: string;
  modifiedAfter: string;
  modifiedBefore: string;
};

const emptyFilters: FileFilters = { extension: "", minSize: "", maxSize: "", modifiedAfter: "", modifiedBefore: "" };

export function useFiles(rootPath: string | null) {
  const [queryText, setQueryText] = useState("");
  const [filters, setFilters] = useState<FileFilters>(emptyFilters);
  const [sortBy, setSortBy] = useState<SearchSortField>("modified");
  const [sortDirection, setSortDirection] = useState<SearchSortDirection>("desc");
  const [page, setPage] = useState(1);
  const [result, setResult] = useState<SearchResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const request = useMemo<SearchQuery | null>(() => {
    if (!rootPath) return null;
    return {
      root_path: rootPath,
      query: queryText,
      extension: filters.extension || null,
      min_size: filters.minSize ? Number(filters.minSize) : null,
      max_size: filters.maxSize ? Number(filters.maxSize) : null,
      modified_after: filters.modifiedAfter ? new Date(filters.modifiedAfter).getTime() : null,
      modified_before: filters.modifiedBefore ? new Date(`${filters.modifiedBefore}T23:59:59.999`).getTime() : null,
      sort_by: sortBy,
      sort_direction: sortDirection,
      page,
      page_size: PAGE_SIZE,
    };
  }, [filters, page, queryText, rootPath, sortBy, sortDirection]);

  const reload = useCallback(async () => {
    if (!request) return;
    setLoading(true);
    try {
      setError(null);
      setResult(await searchFiles(request));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "读取文件索引失败。");
    } finally {
      setLoading(false);
    }
  }, [request]);

  useEffect(() => {
    if (!request) {
      setResult(null);
      return;
    }
    const timer = window.setTimeout(() => void reload(), 150);
    return () => window.clearTimeout(timer);
  }, [reload, request]);

  useEffect(() => {
    setQueryText("");
    setFilters(emptyFilters);
    setSortBy("modified");
    setSortDirection("desc");
    setPage(1);
  }, [rootPath]);

  function updateFilters(next: Partial<FileFilters>) {
    setFilters((current) => ({ ...current, ...next }));
    setPage(1);
  }

  function updateSort(nextSortBy: SearchSortField) {
    if (nextSortBy === sortBy) setSortDirection((current) => current === "asc" ? "desc" : "asc");
    else {
      setSortBy(nextSortBy);
      setSortDirection("asc");
    }
    setPage(1);
  }

  return {
    queryText,
    setQueryText: (value: string) => { setQueryText(value); setPage(1); },
    filters,
    updateFilters,
    sortBy,
    sortDirection,
    updateSort,
    page,
    setPage,
    result,
    loading,
    error,
    reload,
  };
}

export type ReturnTypeUseFiles = ReturnType<typeof useFiles>;
