export type ScanMode = "incremental" | "rebuild";

export type ScanProgress = {
  rootPath: string;
  visitedEntries: number;
  indexedEntries: number;
  errorCount: number;
  currentPath: string | null;
  phase: "scanning" | "persisting" | "completed" | "failed";
};

export type ScanSummary = {
  root_path: string;
  mode: ScanMode;
  indexed_files: number;
  indexed_directories: number;
  indexed_links: number;
  added: number;
  updated: number;
  removed: number;
  ignored: number;
  errors: number;
  completed_at: string;
};

export type IndexStatus = {
  root_path: string;
  indexed_entries: number;
  last_scan_at: string | null;
  state: "empty" | "ready" | "scanning" | "error";
};
