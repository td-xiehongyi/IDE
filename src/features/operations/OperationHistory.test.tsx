import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { OperationHistory } from "./OperationHistory";
import type { OperationHistoryItem } from "../../types/operations";

const item: OperationHistoryItem = {
  id: 1,
  batchId: "batch-1",
  action: "execute",
  operation: "move",
  sourcePath: "C:/Docs/source.txt",
  targetPath: "C:/Docs/archive/source.txt",
  status: "succeeded",
  reason: null,
  createdAt: "100",
  undoStatus: "available",
  undoReason: null,
};

it("shows successful history and exposes undo only when available", () => {
  const onUndo = vi.fn();
  render(<OperationHistory items={[item]} onUndo={onUndo} busy={false} />);

  expect(screen.getByText(/C:\/Docs\/archive\/source\.txt/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "撤销" }));
  expect(onUndo).toHaveBeenCalledWith(1);
});
