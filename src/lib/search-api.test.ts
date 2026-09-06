import { beforeEach, expect, it, vi } from "vitest";

import { listExplorerChildren, resolveIndexedFile } from "./search-api";

const tauri = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => tauri);

beforeEach(() => tauri.invoke.mockReset());

it("uses narrow explorer commands with structured queries", async () => {
  tauri.invoke.mockResolvedValue([]);
  await listExplorerChildren({ root_path: "C:/Docs", parent_path: null });
  expect(tauri.invoke).toHaveBeenCalledWith("list_explorer_children", {
    query: { root_path: "C:/Docs", parent_path: null },
  });

  await resolveIndexedFile({ root_path: "C:/Docs", file_path: "C:/Docs/a.md" });
  expect(tauri.invoke).toHaveBeenCalledWith("resolve_indexed_file", {
    query: { root_path: "C:/Docs", file_path: "C:/Docs/a.md" },
  });
});
