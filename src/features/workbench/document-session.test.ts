import { describe, expect, it } from "vitest";

import type { ExplorerNode } from "../../types/search";
import { closeDocument, emptyDocumentSession, openDocument } from "./document-session";

const node = (name: string): ExplorerNode => ({
  id: name.length,
  normalized_path: `C:/Docs/${name}`,
  name,
  extension: name.split(".").at(-1) ?? null,
  kind: "file",
  size: 1,
  modified_ms: 1,
  has_children: false,
});

describe("document session", () => {
  it("replaces a temporary preview and pins on double open", () => {
    let session = openDocument(emptyDocumentSession(), node("a.md"), false);
    session = openDocument(session, node("b.md"), false);
    expect(session.tabs.map((tab) => tab.entry.name)).toEqual(["b.md"]);
    session = openDocument(session, node("b.md"), true);
    session = openDocument(session, node("c.md"), false);
    expect(session.tabs.map((tab) => [tab.entry.name, tab.pinned])).toEqual([
      ["b.md", true],
      ["c.md", false],
    ]);
  });

  it("keeps at most twenty tabs and selects a neighbor when closing", () => {
    let session = emptyDocumentSession();
    for (let index = 0; index < 21; index += 1) session = openDocument(session, node(`${index}.txt`), true);
    expect(session.tabs).toHaveLength(20);
    expect(session.tabs[0].entry.name).toBe("1.txt");
    session = closeDocument(session, "C:/Docs/20.txt");
    expect(session.activePath).toBe("C:/Docs/19.txt");
  });
});
