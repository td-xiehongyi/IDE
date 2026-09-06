import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { OperationPanel } from "./OperationPanel";
import type { SearchEntry } from "../../types/search";

const entries: SearchEntry[] = [
  { id: 1, normalized_path: "C:/Docs/one.txt", name: "one.txt", extension: "txt", kind: "file", size: 1, modified_ms: 1 },
  { id: 2, normalized_path: "C:/Docs/two.txt", name: "two.txt", extension: "txt", kind: "file", size: 1, modified_ms: 1 },
];

it("builds a batch move draft from selected files", () => {
  const onPreview = vi.fn();
  render(<OperationPanel rootPath="C:/Docs" selectedEntries={entries} onPreview={onPreview} busy={false} />);

  fireEvent.change(screen.getByLabelText("移动到目录"), { target: { value: "C:/Docs/archive" } });
  fireEvent.click(screen.getByRole("button", { name: "生成预览" }));

  expect(onPreview).toHaveBeenCalledWith({
    root_path: "C:/Docs",
    items: [
      { operation: "move", source_path: "C:/Docs/one.txt", destination_directory: "C:/Docs/archive" },
      { operation: "move", source_path: "C:/Docs/two.txt", destination_directory: "C:/Docs/archive" },
    ],
  });
});
