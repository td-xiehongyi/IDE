> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段十设计：受控知识工作 Agent

| 属性 | 值 |
| --- | --- |
| 状态 | 设计完成；未实现 |
| 目标版本 | `v0.2.0` |
| 最后更新 | `2026-08-23` |
| 依赖 | [阶段八 RAG](./PHASE_08_RAG.md)；[阶段九安全编辑](./PHASE_09_DOCUMENT_EDITING.md)；阶段四安全操作管线；[安全模型](./SAFETY_MODEL.md) |

## 目标

- 在 Rust 中运行只面向当前知识空间的受控 Agent。
- 通过固定工具白名单完成知识搜索、文档读取和写操作提案。
- 所有写工具只生成草稿、差异或路径预览；只有用户能批准执行。
- 对每个模型步骤、工具调用、审批与最终状态建立本地审计。
- 在取消、超时、重启或工具错误后停止旧运行，不遗留可继续执行的写计划。

## 非目标

- 不是通用电脑 Agent，不开放 Shell、脚本、终端、任意进程、网络、浏览器或桌面控制。
- 不删除、覆盖、跨卷移动、创建任意目录或操作知识空间外文件。
- 不允许模型直接调用阶段四执行器、阶段九编辑执行器、计划存储、撤销服务或数据库连接。
- 不向模型提供 `planId`、`editPlanId`、审批凭据或可直接执行的绝对路径。
- 不保存模型隐藏推理过程。

## 用户流程与状态机

```text
用户输入目标并确认 ContextScope
  → Queued
  → Running：模型提出白名单工具调用
  → Rust 校验工具名、JSON Schema、知识空间权限和资源预算
  → 只读工具返回受限结果；写工具只生成 Proposal
  → WaitingForApproval：右侧显示摘要，中间显示完整差异或路径预览
  → 用户批准、拒绝或取消
  → 批准后由普通用户流程生成并确认一次性计划
  → Running 或 Completed / Failed / Cancelled
```

状态机：

```text
Queued → Running → WaitingForApproval → Running → Completed
   └──────────────→ Failed
   └──────────────→ Cancelled
```

`Completed`、`Failed`、`Cancelled` 为终态。应用启动时，任何未终结运行都恢复为失败或取消审计状态，不自动继续模型步骤、排队工具或旧审批。

## 运行与审计类型

```ts
type AgentRunState =
  | "Queued"
  | "Running"
  | "WaitingForApproval"
  | "Completed"
  | "Failed"
  | "Cancelled";

type AgentRun = {
  id: string;
  spaceId: string;
  goal: string;
  scope: ContextScope;
  state: AgentRunState;
  stepCount: number;
  deadlineAt: string;
};

type AgentToolCall = {
  id: string;
  runId: string;
  toolName: AgentToolName;
  argumentsDigest: string;
  state: "requested" | "validated" | "completed" | "rejected" | "cancelled";
  resultSummary?: string;
};

type ApprovalRequest = {
  id: string;
  runId: string;
  proposalKind: "document_create" | "document_patch" | "file_rename" | "file_move";
  summary: string;
  previewReference: string;
  state: "pending" | "approved" | "rejected" | "expired" | "cancelled";
};
```

SQLite 规划表为 `agent_runs`、`agent_steps`、`agent_tool_calls` 和 `approval_requests`。审计保存模型/提示词版本、工具名、脱敏封闭结构化参数、知识空间 ID、相关文档 ID、权限决定、状态迁移、时间、结果状态和审批事件；不保存隐藏推理、完整文档正文、绝对路径或一次性计划 ID。

工具参数审计采用逐工具字段白名单：只保留稳定 ID、枚举、布尔值、受长度限制的查询/标题摘要和内容摘要哈希；拒绝并截断额外字段。每条审计记录必须能关联 `runId`、`stepId`、`toolCallId`、`spaceId` 与允许的 `documentId`，并记录请求、校验、完成/拒绝/取消状态以及对应权限决策。审批审计记录提案类型、预览引用、用户动作、动作时间和批准前后状态，不记录差异全文。

## 工具白名单

| 工具 | 权限 | 结果边界 |
| --- | --- | --- |
| `knowledge.search` | 只读 | 只在当前空间和运行范围内检索，返回有限引用块 |
| `document.read` | 只读 | 按文档 ID 和受限位置读取，限制字符数，不接受任意路径 |
| `document.propose_create` | 提案 | 只创建内存中的 Markdown/TXT 草稿，不写磁盘、不创建目录 |
| `document.propose_patch` | 提案 | 针对当前版本生成差异，不获得 `editPlanId` |
| `file.propose_rename` | 提案 | 生成阶段四可校验的重命名草案，不获得 `planId` |
| `file.propose_move` | 提案 | 目标只能是当前空间内已授权、策略允许的现有目录 |

工具参数使用封闭 JSON Schema：拒绝额外字段、未知枚举、绝对路径、父目录跳转、未授权文档 ID、超长字符串和超量项目。工具调度器只识别上述固定名称，不根据模型文本动态加载工具。

`document.propose_create` 首版仅生成未保存标签页草稿。由于阶段九不开放新文件创建，用户可以复制内容或等待独立的新建文档安全规范；不得把“批准提案”误解为已经写入磁盘。

## 执行预算与并发

- 单次运行最多 8 个模型步骤、5 分钟。
- 一个知识空间同时只允许一个活跃 Agent。
- 每次工具调用设置独立超时、最大结果字符数和候选数量。
- 达到任一步数、时间、结果或上下文预算时进入明确失败状态，不自动扩大预算。
- 取消令牌贯穿模型请求、工具队列和结果处理；取消后不得启动下一个工具。
- 模型重试次数固定且计入 8 步上限，不能通过重试绕过预算。

## 审批与写入边界

- Agent 写工具的唯一产物是 `ApprovalRequest` 及其预览引用。
- 右侧展示目标、原因、影响文件和摘要；中间展示完整差异或 From/To 路径预览。
- 用户批准只表示同意把提案交给正常手动流程。实际文件写入仍须由 Rust 生成新的 `planId` 或 `editPlanId`，并要求用户在对应预览中再次明确确认。
- 模型工具结果只收到“提案已创建/已拒绝/已过期”等状态，不收到计划 ID、内部路径快照或确认接口。
- 审批过期、文件变化、知识空间切换或 Agent 取消都会使提案失效。
- 执行历史沿用阶段四或阶段九记录；Agent 审计通过关联 ID 记录提案来源，但不能执行撤销。

## 规划接口与事件

| 领域入口 | 作用 |
| --- | --- |
| `agent/start` | 校验目标、范围和并发后创建运行 |
| `agent/get`、`agent/list` | 查询当前空间运行与审计摘要 |
| `agent/cancel` | 传播取消并终止排队工具 |
| `agent/approve-proposal` | 由用户界面批准提案进入普通预览流程 |
| `agent/reject-proposal` | 记录用户拒绝并使提案终结 |

事件为 `agent://run-updated` 和 `agent://approval-required`。实际 Command 必须固定注册，不能提供通用工具执行 Command。

## 错误与安全

- `unknown_tool`、`invalid_arguments`、`scope_violation`：Rust 拒绝并审计，模型不能自行放宽 Schema。
- `approval_required`：写提案只能等待用户处理，不能自动批准。
- `document_changed`：提案依据版本过期，要求重新读取和重新提案。
- `budget_exceeded`、`timeout`、`cancelled`：终止运行并阻止后续工具。
- `provider_error`、`tool_error`：记录有限错误摘要，不泄露正文或敏感路径。
- 文档内提示注入一律视为不可信内容，不能修改系统权限、工具列表、范围、预算或审批规则。
- Ollama 的结构化工具调用结果只是未信任输入，Rust 必须独立校验工具名、参数、授权、状态和输出上限。

## 测试矩阵

- 状态机：正常完成、等待审批、拒绝、过期、失败、取消、重启和非法迁移。
- 工具：全部白名单成功路径；未知工具、额外字段、绝对路径、越界文档和超限结果。
- 权限：跨知识空间、链接/Junction、目标目录失效、文件变化和未授权根目录。
- 审批：零审批写入、批准后仍需普通确认、计划 ID 不进入模型上下文、重复审批和空间切换。
- 预算：第 8 步、5 分钟、工具超时、取消传播、单空间并发和模型重试计数。
- 审计：可还原请求、工具、Rust 决策与用户批准；不含正文、隐藏推理或计划 ID。
- 审计脱敏：参数字段白名单、长度限制、知识空间/文档 ID 关联、权限决策、状态迁移和审批事件完整可查询。
- 提示注入：文档要求调用 Shell、泄露路径、改变工具或跳过审批时均被忽略和拦截。

## 验收门槛

- 未经用户批准的 Agent 写操作执行数为零。
- 未授权路径、未知工具和非法参数拦截率 100%。
- 模型上下文、工具结果、SQLite 与日志中的 `planId` / `editPlanId` 暴露数为零。
- 取消、超时或应用重启后继续旧模型步骤或排队工具的数量为零。
- 一个知识空间不出现两个活跃 Agent。
- 审计记录能够通过脱敏封闭参数、知识空间/文档 ID、权限决策、状态迁移和审批事件还原模型请求版本、工具调用与用户审批结果。

这些是未来实现门槛；当前仓库没有受控 Agent 运行时。

## 已知限制

- 首版只能顺序执行有限步骤，不支持多 Agent、后台长期任务或跨空间计划。
- `document.propose_create` 只生成内存草稿，不写磁盘。
- Agent 质量依赖阶段八检索与本地模型能力，但安全边界不能因模型能力调整而放宽。
- 远程模型、网络工具和桌面控制不在本路线范围。
