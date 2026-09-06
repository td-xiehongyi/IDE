import { describe, expect, it } from "vitest";

import { emptyExplorerState, selectExplorerNode, setExplorerChildren } from "./explorer-state";
import type { ExplorerNode } from "../../types/search";

const file = (name: string): ExplorerNode => ({ id: name.length, normalized_path: `C:/Docs/${name}`, name, extension: "txt", kind: "file", size: 1, modified_ms: 1, has_children: false });

describe("explorer state", () => {
  it("stores lazy children by parent", () => {
    const state = setExplorerChildren(emptyExplorerState(), "C:/Docs", [file("a.txt")]);
    expect(state.childrenByParent["C:/Docs"][0].name).toBe("a.txt");
  });

  it("supports replace, ctrl toggle and shift range selection", () => {
    const nodes = [file("a.txt"), file("b.txt"), file("c.txt")];
    let state = selectExplorerNode(emptyExplorerState(), nodes[0], "replace", nodes);
    state = selectExplorerNode(state, nodes[2], "toggle", nodes);
    expect([...state.selectedPaths]).toEqual(["C:/Docs/a.txt", "C:/Docs/c.txt"]);
    state = selectExplorerNode(state, nodes[1], "range", nodes);
    expect([...state.selectedPaths]).toEqual(["C:/Docs/a.txt", "C:/Docs/b.txt"]);
  });
});
