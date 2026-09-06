import { invoke } from "@tauri-apps/api/core";

import type {
  OperationBatchResult,
  OperationDraft,
  OperationHistoryItem,
  OperationPreviewResponse,
  OperationResultItem,
} from "../types/operations";

type PreviewWire = {
  can_confirm: boolean;
  items: Array<{
    index: number;
    operation: "move" | "rename";
    source_path: string;
    target_path: string;
    status: "valid" | "invalid";
    reason: string | null;
    will_create_directory: boolean;
  }>;
  plan_id: string | null;
  expires_at: string | null;
};

type ResultWire = {
  index: number;
  operation: "move" | "rename";
  source_path: string;
  target_path: string;
  status: "succeeded" | "failed" | "not_executed";
  reason: string | null;
  history_id: number | null;
};

type HistoryWire = {
  id: number;
  batch_id: string;
  action: "execute" | "undo";
  operation: "move" | "rename";
  source_path: string;
  target_path: string;
  status: "succeeded" | "failed" | "not_executed";
  reason: string | null;
  created_at: string;
  undo_status: "available" | "unavailable" | "undone";
  undo_reason: string | null;
};

export async function previewOperations(draft: OperationDraft): Promise<OperationPreviewResponse> {
  const response = await invoke<PreviewWire>("preview_operations", { draft });
  return {
    canConfirm: response.can_confirm,
    planId: response.plan_id,
    expiresAt: response.expires_at,
    items: response.items.map((item) => ({
      index: item.index,
      operation: item.operation,
      sourcePath: item.source_path,
      targetPath: item.target_path,
      status: item.status,
      reason: item.reason,
      willCreateDirectory: item.will_create_directory,
    })),
  };
}

export function cancelOperationPlan(planId: string): Promise<void> {
  return invoke("cancel_operation_plan", { planId });
}

export async function executeOperationPlan(planId: string): Promise<OperationBatchResult> {
  const response = await invoke<{ batch_id: string; items: ResultWire[] }>("execute_operation_plan", { planId });
  return { batchId: response.batch_id, items: mapResultItems(response.items) };
}

export async function getOperationHistory(limit = 50, offset = 0): Promise<OperationHistoryItem[]> {
  const response = await invoke<HistoryWire[]>("get_operation_history", { limit, offset });
  return response.map((item) => ({
    id: item.id,
    batchId: item.batch_id,
    action: item.action,
    operation: item.operation,
    sourcePath: item.source_path,
    targetPath: item.target_path,
    status: item.status,
    reason: item.reason,
    createdAt: item.created_at,
    undoStatus: item.undo_status,
    undoReason: item.undo_reason,
  }));
}

export async function undoOperation(historyId: number): Promise<OperationResultItem> {
  const response = await invoke<ResultWire>("undo_operation", { historyId });
  return mapResultItem(response);
}

function mapResultItems(items: ResultWire[]): OperationResultItem[] {
  return items.map(mapResultItem);
}

function mapResultItem(item: ResultWire): OperationResultItem {
  return {
    index: item.index,
    operation: item.operation,
    sourcePath: item.source_path,
    targetPath: item.target_path,
    status: item.status,
    reason: item.reason,
    historyId: item.history_id,
  };
}
