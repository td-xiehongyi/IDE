> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段九设计：Markdown/TXT 安全编辑与版本恢复

| 属性 | 值 |
| --- | --- |
| 状态 | 设计完成；未实现 |
| 目标版本 | `v0.2.0` |
| 最后更新 | `2026-08-23` |
| 依赖 | [阶段六工作台](./PHASE_06_WORKBENCH_UI.md)；[阶段七知识核心](./PHASE_07_KNOWLEDGE_CORE.md)；[安全模型](./SAFETY_MODEL.md) |

## 目标

- 仅为授权知识空间内的现有 Markdown 与 TXT 普通文件提供手动编辑。
- 保存前显示差异，由 Rust 复核文件身份、内容指纹、权限、编码和目标状态。
- 使用短期、一次性的 `editPlanId` 把预览内容与实际写入绑定。
- 保存原始版本快照，采用同目录临时文件与平台验证后的原子替换，并在成功后触发增量索引。
- 版本恢复也经过预览、确认和执行前复核。

## 非目标

- 不自动保存，不提供后台静默改写。
- 不编辑 PDF、DOCX、代码、配置、二进制、链接或目录。
- 不覆盖外部修改，不进行自动三方合并或冲突自动选择。
- 前端只能在直接响应用户对当前差异的明确确认动作时提交一次性 `editPlanId`；前端业务逻辑、AI 和 Agent 不得自行确认、访问计划存储或替换已预览内容。
- 不在本阶段开放任意新文件创建、删除、目录创建、跨卷写入或 Shell。

## 用户流程

```text
打开 Markdown/TXT
  → Rust 返回正文、编码、BOM、换行和文件身份快照
  → 用户进入编辑模式并修改内存草稿
  → Ctrl+S 或保存
  → 前端提交草稿与基线版本
  → Rust 复核授权、普通文件、资源限制和外部变化
  → 生成差异、预览与 editPlanId
  → 用户检查完整差异并明确确认
  → Rust 消费 editPlanId 并再次复核
  → 保存原始版本快照
  → 同目录临时文件写入、刷新并原子替换
  → 记录编辑历史并触发增量索引
```

关闭标签、切换知识空间或退出应用时，未保存草稿必须提示保存、放弃或取消；不得静默丢失或自动写盘。

## 类型与状态

```ts
type DocumentEditDraft = {
  spaceId: string;
  documentId: string;
  baseFingerprint: string;
  content: string;
};

type DocumentDiff = {
  format: "unified";
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
};

type DocumentEditPreview = {
  editPlanId: string;
  expiresAt: string;
  documentId: string;
  diff: DocumentDiff;
  encoding: "utf-8" | "utf-16le" | "utf-16be";
  hasBom: boolean;
  lineEnding: "lf" | "crlf" | "cr";
};

type DocumentVersion = {
  id: string;
  documentId: string;
  createdAt: string;
  sourceFingerprint: string;
  byteLength: number;
};

type DocumentEditResult = {
  status: "succeeded" | "failed";
  documentId: string;
  previousVersionId?: string;
  newFingerprint?: string;
  errorCode?: string;
};
```

计划状态为 `valid`、`consumed`、`cancelled`、`expired`；除 `valid` 外均为终态。有效期固定 10 分钟，确认开始时立即消费，应用退出后失效。

## 规划接口

| 领域入口 | 输入 | 输出与边界 |
| --- | --- | --- |
| `document/read-editable` | 知识空间和文档 ID | 返回受限正文与基线快照，只允许 Markdown/TXT |
| `document-edit/preview` | `DocumentEditDraft` | 复核后返回差异与 `editPlanId`，不写源文件 |
| `document-edit/cancel` | `editPlanId` | 使有效计划终止 |
| `document-edit/execute` | `editPlanId` | 消费后复核、快照、受控写入和历史记录 |
| `document-edit/list-versions` | 文档 ID | 返回版本元数据，不自动读取全部快照 |
| `document-edit/preview-restore` | 版本 ID 与当前文档 ID | 生成恢复差异和新的 `editPlanId` |

领域入口最终必须实现为逐个固定注册的 Tauri Command，不能提供接受任意命令名或任意路径的通用入口。执行请求只提交 `editPlanId`，不能重新提交路径或内容；该提交必须直接对应用户对当前差异预览的明确确认动作。

## 保存数据流与持久化

- 可编辑读取记录规范化路径、普通文件类型、大小、修改时间、平台文件 ID、内容指纹、编码、BOM 与换行。
- `preview` 只对基线完全匹配的文档生成差异；空变化不生成计划。
- 版本快照保存在应用数据目录的专用、非扫描区域；SQLite 只保存版本元数据、受控快照定位和编辑审计。
- 快照写入成功后，才在源文件同目录创建不可预测名称的临时文件；临时文件必须使用排他创建并继承经过审查的必要权限。
- 临时文件完整写入并刷新后，使用当前平台已验证的替换方式提交。替换失败不得声称成功；可识别的临时文件由受控恢复流程清理。
- 成功后计算新指纹、写入编辑历史、发出 `document://external-change` 或索引更新事件，并使旧知识索引立即过期。
- 历史审计不保存用户未确认的草稿；版本快照保存完整原始字节，以准确保留编码、BOM 和换行。

## 编码与内容规则

- 首版支持 UTF-8、UTF-8 BOM、UTF-16LE BOM、UTF-16BE BOM；无 BOM 且无法可靠解码时拒绝编辑。
- 保存保留原编码、BOM 和主要换行风格，不在未提示时规范化全文。
- 新内容必须满足配置的字节与字符上限；超限在预览前拒绝。
- 文件扩展名必须保持 `.md` 或 `.txt`，本阶段编辑保存不包含重命名。
- 差异按行展示，并为过长行、二进制特征或控制字符给出明确错误。

## 错误与安全

- `document_changed`：基线后大小、时间、平台身份或指纹变化；拒绝覆盖并要求重新加载。
- `unsupported_encoding` / `unsupported_type`：不可编辑，仍允许只读查看。
- `unauthorized_path` / `linked_file`：文件不在当前知识空间真实授权范围或为链接，拒绝读取编辑正文。
- `plan_expired` / `plan_consumed`：不能执行，必须重新预览。
- `snapshot_failed`：不进入源文件写入。
- `temporary_write_failed` / `atomic_replace_failed`：返回真实失败，不记录成功历史；保留或清理临时文件须可审计。
- `permission_changed`：执行前权限变化，拒绝写入。

前端业务逻辑、AI 和 Agent 都不能读取计划存储、自行确认计划或替换已预览内容。只有用户明确点击确认后，前端才可提交当前一次性 `editPlanId`。该 ID 不写入 SQLite、对话、工具结果或日志。

## 测试矩阵

- 类型：Markdown/TXT 成功，PDF/DOCX/代码/目录/链接拒绝。
- 冲突：同大小替换、同时间替换、身份变化、权限变化、外部删除和路径被链接替换。
- 编码：UTF-8、BOM、UTF-16、LF/CRLF/CR、混合换行和不可解码字节。
- 计划：10 分钟过期、取消、重复确认、应用重启和执行请求内容不可替换。
- 写入：快照失败、临时文件失败、刷新失败、替换失败、成功后真实字节与指纹。
- 版本：列表、恢复差异、恢复冲突、重复恢复和快照缺失。
- 索引：成功编辑使旧索引失效并触发增量任务；失败不切换索引版本。
- UI：脏状态、关闭/切换/退出提示、完整差异、键盘保存和错误恢复。

## 验收门槛

- 未确认或无有效 `editPlanId` 时源文件写入次数为零。
- 外部修改、链接替换、权限变化、计划过期和重复确认均不会覆盖当前文件。
- 成功保存保留原编码、BOM 和换行风格，并能从版本快照恢复原始字节。
- 每次成功编辑有真实审计记录并使旧知识索引失效；失败不产生成功历史。
- Agent 无法获得、确认或消费 `editPlanId`。
- 所有文件系统测试在隔离临时目录验证真实结果。

## 已知限制

- 首版不提供自动合并、协同编辑、自动保存或任意文件创建。
- 不同平台的原子替换、权限和防病毒占用行为需要分别验收。
- 大文件差异性能与版本保留配额需在阶段开始前锁定。
- 版本快照包含本地文档原始字节，必须提供清除、配额与隐私说明。
