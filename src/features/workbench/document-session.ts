import type { ExplorerNode } from "../../types/search";

export type DocumentTab = {
  entry: ExplorerNode;
  pinned: boolean;
  stale: boolean;
};

export type DocumentSession = {
  tabs: DocumentTab[];
  activePath: string | null;
};

export function emptyDocumentSession(): DocumentSession {
  return { tabs: [], activePath: null };
}

export function openDocument(session: DocumentSession, entry: ExplorerNode, pinned: boolean): DocumentSession {
  const path = entry.normalized_path;
  let tabs = session.tabs.filter((tab) => tab.pinned || tab.entry.normalized_path === path);
  const existing = tabs.find((tab) => tab.entry.normalized_path === path);
  if (existing) {
    tabs = tabs.map((tab) => tab.entry.normalized_path === path ? { ...tab, entry, pinned: tab.pinned || pinned, stale: false } : tab);
  } else {
    tabs = [...tabs, { entry, pinned, stale: false }];
  }
  if (tabs.length > 20) tabs = tabs.slice(tabs.length - 20);
  return { tabs, activePath: path };
}

export function closeDocument(session: DocumentSession, path: string): DocumentSession {
  const index = session.tabs.findIndex((tab) => tab.entry.normalized_path === path);
  const tabs = session.tabs.filter((tab) => tab.entry.normalized_path !== path);
  if (session.activePath !== path) return { ...session, tabs };
  const activePath = tabs[Math.max(0, index - 1)]?.entry.normalized_path ?? tabs[0]?.entry.normalized_path ?? null;
  return { tabs, activePath };
}
