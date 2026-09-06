import type { IndexStatus, ScanProgress, ScanSummary } from "../../types/files";
import type { ExplorerNode } from "../../types/search";
import type { DocumentSession } from "./document-session";
import type { ExplorerState } from "./explorer-state";

export type AssistantMode = "ask" | "agent";
export type ActivityMode = "index" | "watcher" | "model" | "tasks" | "history";
export type DocumentViewerKind = "metadata" | "operation-preview";

export type DocumentLocation = {
  rootPath: string;
  normalizedPath: string;
};

export type WorkspaceSession = {
  rootPath: string | null;
  status: IndexStatus | null;
  progress: ScanProgress | null;
  summary: ScanSummary | null;
  busy: boolean;
  error: string | null;
};

export type AssistantSession = {
  mode: AssistantMode;
  context: ExplorerNode[];
  providerMessage: string;
};

export type ActivityState = {
  mode: ActivityMode;
  collapsed: boolean;
};

export type { DocumentSession, ExplorerState };
