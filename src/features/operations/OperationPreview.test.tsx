import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { OperationPreview } from "./OperationPreview";
import type { OperationPreviewResponse } from "../../types/operations";

const validPreview: OperationPreviewResponse = {
  canConfirm: true,
  planId: "plan-123",
  expiresAt: "9999999999",
  items: [
    {
      index: 0,
      operation: "move",
      sourcePath: "C:/Docs/source.txt",
      targetPath: "C:/Docs/archive/source.txt",
      status: "valid",
      reason: null,
      willCreateDirectory: false,
    },
  ],
};

it("shows From/To and confirms only with the plan id", () => {
  const onConfirm = vi.fn();
  render(
    <OperationPreview
      preview={validPreview}
      onConfirm={onConfirm}
      onCancel={vi.fn()}
      busy={false}
    />,
  );

  expect(screen.getByText("C:/Docs/source.txt")).toBeInTheDocument();
  expect(screen.getByText("C:/Docs/archive/source.txt")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "确认执行" }));
  expect(onConfirm).toHaveBeenCalledWith("plan-123");
  expect(onConfirm.mock.calls[0]).toHaveLength(1);
});

it("does not expose a confirm action for an invalid preview", () => {
  render(
    <OperationPreview
      preview={{
        ...validPreview,
        canConfirm: false,
        planId: null,
        items: [{ ...validPreview.items[0], status: "invalid", reason: "目标已存在" }],
      }}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
      busy={false}
    />,
  );

  expect(screen.queryByRole("button", { name: "确认执行" })).not.toBeInTheDocument();
  expect(screen.getByText("目标已存在")).toBeInTheDocument();
});
