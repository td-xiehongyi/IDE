import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ExplorerChildrenQuery, ExplorerNode, IndexedFileQuery, SearchQuery, SearchResult } from "../types/search";

export function searchFiles(query: SearchQuery): Promise<SearchResult> {
  return invoke<SearchResult>("search_files", { query });
}

export function listExplorerChildren(query: ExplorerChildrenQuery): Promise<ExplorerNode[]> {
  return invoke<ExplorerNode[]>("list_explorer_children", { query });
}

export function resolveIndexedFile(query: IndexedFileQuery): Promise<ExplorerNode | null> {
  return invoke<ExplorerNode | null>("resolve_indexed_file", { query });
}

export function listenForIndexChanges(callback: () => void): Promise<UnlistenFn> {
  return listen("files://index-changed", callback);
}

export function listenForWatcherErrors(callback: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("files://watcher-error", (event) => callback(event.payload));
}
