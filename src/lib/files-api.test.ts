import { expect, it, vi } from "vitest";

import { chooseFile } from "./files-api";

const dialog = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => dialog);

it("opens a single-file native picker", async () => {
  dialog.open.mockResolvedValue("C:/Docs/a.md");
  await expect(chooseFile()).resolves.toBe("C:/Docs/a.md");
  expect(dialog.open).toHaveBeenCalledWith({ directory: false, multiple: false, title: "打开已索引文件" });
});
