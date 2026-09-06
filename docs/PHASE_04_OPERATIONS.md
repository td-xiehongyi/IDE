> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段四开发指南：安全文件操作、历史与撤销

| 属性 | 值 |
| --- | --- |
| 状态 | 已实现 |
| 对应版本 | `v0.1.0` |
| 文档更新 | `2026-08-22` |

## 目的与边界

阶段四为手动文件整理提供唯一的写入通道。它只支持当前授权根目录内的普通文件：可批量移动到已存在的同卷目录，或对单个文件重命名。所有路径都先经 Rust 校验并形成短期计划；前端不能直接调用文件系统，也不能在确认时替换已预览的路径。

本阶段明确不提供删除、覆盖、目录操作、创建目录、跨卷复制或 Shell 接口。AI 建议尚未实现；阶段五若创建草案，仍必须复用本阶段的预览、确认和执行管线。

规范性安全规则见[安全模型](./SAFETY_MODEL.md)。

## 对外接口与数据流

前端通过 `src/lib/operations-api.ts` 调用以下窄化 Tauri Command：

| Command | 输入 | 输出与效果 |
| --- | --- | --- |
| `preview_operations` | `OperationDraft` | 返回逐项 From/To 与校验结果；仅全部有效时返回 `planId` 和 10 分钟有效期；不写磁盘。 |
| `cancel_operation_plan` | `planId` | 将有效计划标为已取消。 |
| `execute_operation_plan` | `planId` | 立即消费计划、逐项复核并执行，返回成功、失败或未执行的真实结果。 |
| `get_operation_history` | `limit`、`offset` | 读取长期历史，并按当前磁盘状态计算撤销资格。 |
| `undo_operation` | `historyId` | 仅在恢复安全时把成功的原始操作恢复到 From 路径。 |

`OperationDraft` 使用蛇形字段，并且每个项目带有显式操作标签：

```ts
type OperationDraft = {
  root_path: string;
  items: (
    | { operation: "move"; source_path: string; destination_directory: string }
    | { operation: "rename"; source_path: string; new_name: string }
  )[];
};
```

执行请求只包含 `planId`。`planId` 不持久化，应用退出后自然失效；无效、取消、过期或已消费的计划均不能再次执行。

```text
文件行选择 / 手动输入
  → OperationDraft
  → preview_operations：根目录、路径、类型、冲突、卷与快照校验
  → From/To 预览 + planId
  → execute_operation_plan(planId)
  → 执行前复核 + fs::rename
  → operation_history + files://index-changed
```

## 后端职责与安全决策

`operations.rs` 负责确认授权根目录仍是 watcher 的当前根目录、管理 Command 响应，并在执行或撤销后发出 `files://index-changed`。它不接受任意路径执行请求。

- `operation_validator` 对整批草案预检。它规范化路径并拒绝越界路径、前缀伪装、目录、符号链接/Junction、缺失目标目录、非法文件名、同路径操作、目标冲突和跨卷移动。
- `file_identity` 为普通文件记录类型、大小、修改时间、卷标识和平台可用的稳定身份（Unix 的设备/节点；Windows 的卷序列号/文件索引）。手动操作不计算内容哈希。
- `plan_store` 将仅完整有效的预览保留在 Rust 内存中；状态为 `Valid`、`Consumed`、`Canceled` 或 `Expired`，有效期固定为 600 秒。
- `operation_executor` 在每项执行前重核源快照、目标不存在和目标父目录存在。首个运行时失败后，后续项目记为 `not_executed`；已成功项目不自动回滚。
- `undo_service` 只处理成功的原始执行记录。它要求原始路径未占用、当前目标仍为匹配快照的普通文件且原始父目录存在；重复撤销、冲突、缺失和身份不匹配都被拒绝。

## 历史与索引

迁移 `002_operation_history.sql` 建立 `operation_history`。每条记录保存批次、动作（`execute`/`undo`）、操作类型、From/To、结果状态、原因、时间、文件快照和 `reverses_id` 关系。

历史是长期数据，文件索引是可重建数据。索引重建只能清理索引表，不能删除操作历史。历史列表中的 `undoStatus` 不是静态字段：每次查询会按磁盘当前状态返回 `available`、`unavailable` 或 `undone`，并在不可用时附带原因。

## 前端集成

`FilesFeature` 维护当前页普通文件的选择、预览、执行状态和历史刷新。`OperationPanel` 仅允许批量移动或单文件重命名；`OperationPreview` 仅在 `canConfirm` 与 `planId` 同时存在时展示确认按钮；`OperationHistory` 只为当前可撤销项目展示撤销入口。

接口映射集中在 `operations-api.ts`，将 Rust 的蛇形响应转换为前端的驼峰类型。新增操作 UI 时应继续通过该层调用，禁止在组件中直接调用任意 Tauri Command 或拼装写入请求。

## 测试与维护

阶段四测试均使用隔离临时目录，并验证真实磁盘状态：

- Rust：路径越界、普通文件限制、链接/Junction、冲突、计划生命周期、预览后源变化、批量运行时失败、历史保留与撤销安全。
- 前端：普通文件选择、移动和重命名草案、无效预览阻止确认、确认只发送 `planId`、结果与历史展示。

修改本管线后至少运行：

```text
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

如果新增任何文件写入能力，先更新[安全模型](./SAFETY_MODEL.md)和本指南，再实现 Command；不得以“便利接口”绕过计划、复核、历史和撤销资格计算。
