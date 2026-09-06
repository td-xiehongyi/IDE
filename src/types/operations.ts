export type OperationType = "move" | "rename";
export type OperationValidationStatus = "valid" | "invalid";
export type OperationResultStatus = "succeeded" | "failed" | "not_executed";
export type HistoryAction = "execute" | "undo";
export type UndoStatus = "available" | "unavailable" | "undone";

export type OperationDraftItem =
  | { operation: "move"; source_path: string; destination_directory: string }
  | { operation: "rename"; source_path: string; new_name: string }
  | { operation: "ai_organize"; source_path: string; category_id: string; new_name: string; content_fingerprint: string }
  | { operation: "ai_rename"; source_path: string; new_name: string; content_fingerprint: string };

export type OperationDraft = { root_path: string; items: OperationDraftItem[] };

export type OperationPreviewItem = {
  index: number;
  operation: OperationType;
  sourcePath: string;
  targetPath: string;
  status: OperationValidationStatus;
  reason: string | null;
  willCreateDirectory: boolean;
};

export type OperationPreviewResponse = {
  canConfirm: boolean;
  items: OperationPreviewItem[];
  planId: string | null;
  expiresAt: string | null;
};

export type OperationResultItem = {
  index: number;
  operation: OperationType;
  sourcePath: string;
  targetPath: string;
  status: OperationResultStatus;
  reason: string | null;
  historyId: number | null;
};

export type OperationBatchResult = { batchId: string; items: OperationResultItem[] };

export type OperationHistoryItem = {
  id: number;
  batchId: string;
  action: HistoryAction;
  operation: OperationType;
  sourcePath: string;
  targetPath: string;
  status: OperationResultStatus;
  reason: string | null;
  createdAt: string;
  undoStatus: UndoStatus;
  undoReason: string | null;
};
