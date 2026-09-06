import type { ExplorerNode } from "../../types/search";

export type ExplorerState = {
  childrenByParent: Record<string, ExplorerNode[]>;
  expandedPaths: Set<string>;
  selectedPaths: Set<string>;
  activePath: string | null;
  anchorPath: string | null;
  filterText: string;
};

export type SelectionMode = "replace" | "toggle" | "range";

export function emptyExplorerState(): ExplorerState {
  return { childrenByParent: {}, expandedPaths: new Set(), selectedPaths: new Set(), activePath: null, anchorPath: null, filterText: "" };
}

export function setExplorerChildren(state: ExplorerState, parentPath: string, children: ExplorerNode[]): ExplorerState {
  return { ...state, childrenByParent: { ...state.childrenByParent, [parentPath]: children } };
}

export function selectExplorerNode(state: ExplorerState, node: ExplorerNode, mode: SelectionMode, visibleNodes: ExplorerNode[]): ExplorerState {
  const path = node.normalized_path;
  if (mode === "toggle") {
    const selectedPaths = new Set(state.selectedPaths);
    selectedPaths.has(path) ? selectedPaths.delete(path) : selectedPaths.add(path);
    return { ...state, selectedPaths, activePath: path, anchorPath: state.anchorPath ?? path };
  }
  if (mode === "range" && state.anchorPath) {
    const start = visibleNodes.findIndex((entry) => entry.normalized_path === state.anchorPath);
    const end = visibleNodes.findIndex((entry) => entry.normalized_path === path);
    if (start >= 0 && end >= 0) {
      const [from, to] = start <= end ? [start, end] : [end, start];
      return { ...state, selectedPaths: new Set(visibleNodes.slice(from, to + 1).filter((entry) => entry.kind === "file").map((entry) => entry.normalized_path)), activePath: path };
    }
  }
  return { ...state, selectedPaths: new Set([path]), activePath: path, anchorPath: path };
}
