import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type { IndexStatus, ScanMode, ScanProgress, ScanSummary } from "../types/files";

export async function chooseDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: "选择扫描目录" });
  return typeof selected === "string" ? selected : null;
}

export async function chooseFile(): Promise<string | null> {
  const selected = await open({ directory: false, multiple: false, title: "打开已索引文件" });
  return typeof selected === "string" ? selected : null;
}

export async function chooseTargetDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: "选择移动目标目录" });
  return typeof selected === "string" ? selected : null;
}

export function scanDirectory(rootPath: string, mode: ScanMode): Promise<ScanSummary> {
  return invoke<ScanSummary>("scan_directory", { rootPath, mode });
}

export function getIndexStatus(rootPath: string): Promise<IndexStatus> {
  return invoke<IndexStatus>("get_index_status", { rootPath });
}

export function restoreRecentIndex(): Promise<IndexStatus | null> {
  return invoke<IndexStatus | null>("restore_recent_index");
}

export function rebuildIndex(): Promise<void> {
  return invoke("rebuild_index");
}

export function listenForScanProgress(callback: (progress: ScanProgress) => void): Promise<UnlistenFn> {
  return listen<ScanProgress>("files://scan-progress", (event) => callback(event.payload));
}
