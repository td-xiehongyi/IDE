> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](../../PRD.md)、[路线图](../../ROADMAP.md)与[历史索引](../../history/README.md)。

# AI 文件整理器向个人知识库与受控 Agent 转型设计

| 属性 | 值 |
| --- | --- |
| 状态 | 设计已批准；未实现 |
| 目标版本 | `v0.2.0` |
| 最后更新 | `2026-08-23` |
| 依赖 | 阶段一至四已实现能力；阶段五内部开发版；[安全模型](../../SAFETY_MODEL.md) |

## 目标

在保留安全文件管理底座的前提下，把 AI File Organizer 渐进升级为本地优先的个人知识库与受控知识工作 Agent。目标产品由四层组成：

1. 已实现的安全文件管理底座；
2. IDE 式知识工作台；
3. 可追溯引用的本地知识检索与问答；
4. 只能通过白名单工具提出变更、由用户批准写入的受控 Agent。

原文件始终是事实来源。SQLite 可以保存用户明确建立知识索引后产生的可重建文本块、向量、会话和审计数据，但不得让派生数据替代执行前的真实文件系统复核。

## 非目标

- 不整体重写现有 Tauri、React、Rust 与 SQLite 工程。
- 不把阶段五内部开发版描述为已发布，也不把阶段六至十的设计描述为已实现。
- 不开放 Shell、任意文件访问、删除、覆盖、跨卷移动、网络服务或桌面自动化。
- 不开放通用目录创建。阶段五仅允许用户确认 AI 分类计划后，由 Rust 在授权根目录直属位置创建或复用合法单层分类目录；AI 与 Agent 不能选择或创建任意目录。
- 不让 AI 或 Agent 获得 `planId`、`editPlanId` 的批准或消费能力。
- 首版知识索引不支持 OCR、XLSX、PPTX、旧版 `.doc` 或多模态内容理解。
- 不把训练指南、嵌入模型候选或评测方案描述为已训练、已接入或已达标。

## 当前基线与阶段状态

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| 一至四 | 已完成 | 工程基础、扫描索引、浏览监听、安全移动/重命名与撤销 |
| 五 | 内部开发版 | 本地 Ollama 内容分析初版；仍有四项已知问题并缺少真实黄金集发布验收 |
| 六 | 设计完成、未实现 | IDE 式知识工作台 UI 与前端领域拆分 |
| 七 | 设计完成、未实现 | 知识空间、正文切片、FTS5 与可替换向量仓储 |
| 八 | 设计完成、未实现 | 混合检索、引用校验与有依据问答 |
| 九 | 设计完成、未实现 | Markdown/TXT 手动安全编辑与版本恢复 |
| 十 | 设计完成、未实现 | Rust 受控 Agent、白名单工具与人工审批 |
| 十一 | 规划中 | 跨平台、性能、安全审计与发布 |

实施前以完整源码仓库为唯一技术基线，保护全部未提交成果。当前 `D:\MyProject` 可承载规范文档，但在确认与完整源码一致前，不把它作为代码实施基线。

## 产品结构与用户流程

```text
顶部：知识空间 / 全局搜索 / 模型状态 / 设置
左侧：多根授权目录的文件树
中间：文档标签页 / 阅读器 / 编辑器 / 差异与路径预览
右侧：Ask / Organize / Agent Run / 引用 / 审批
底部：索引 / 监听 / 模型 / 后台任务 / 操作历史
```

核心用户流程：

```text
创建知识空间并明确选择一个或多个真实目录
  → 只读扫描与知识索引
  → 在工作台打开、搜索和定位文档
  → 选择 CurrentDocument / SelectedDocuments / KnowledgeSpace 范围提问
  → Rust 混合检索并只向模型提供带 chunkId 的证据
  → 校验引用后展示回答，点击引用定位原文
  → 用户手动编辑 Markdown/TXT，或让 Agent 提出草稿
  → Rust 生成差异或路径预览
  → 用户明确批准一次性计划
  → 执行前复核、受控写入、记录历史并触发增量索引
```

## 领域与状态边界

前端拆分为：

```text
app-shell
workspaces
explorer
documents
search
operations
knowledge-index
assistant
agent
approvals
activity
settings
```

会话状态：

- `WorkspaceSession`：当前知识空间、根目录与恢复状态。
- `ExplorerState`：展开节点、选中文件、过滤与搜索。
- `DocumentSession`：标签页、活动文档、草稿与脏状态。
- `AssistantSession`：对话、上下文范围与引用。
- `AgentSession`：运行状态、工具调用与待审批事项。
- `ActivityState`：扫描、索引、文件操作和 Agent 后台任务。

面板宽度、折叠状态和最近标签页属于 UI 会话；知识空间、索引、会话、编辑版本、审批与 Agent 审计属于本地持久状态。React 不直接访问 SQLite、文件系统或 Ollama。

## 领域模型与公共接口

规划中的核心模型：

```text
KnowledgeSpace / KnowledgeRoot / KnowledgeDocument / DocumentIdentity
IndexState / IndexJob / DocumentChunk / ChunkEmbedding
DocumentLocation / DocumentViewer
KnowledgeQuery / ContextScope / RetrievedChunk / Citation / GroundedAnswer
Conversation / ChatMessage
DocumentEditDraft / DocumentDiff / DocumentEditPreview / DocumentVersion
AgentRun / AgentStep / AgentToolCall / ApprovalRequest
```

Tauri Command 按领域提供窄化入口：

```text
workspace/*
document/*
knowledge-index/*
retrieval/*
conversation/*
document-edit/*
agent/*
operations/*
```

这里的斜杠表示文档中的领域分组，不要求暴露接受任意命令名的通用路由。每个实际 Command 必须是固定名称、固定输入和固定输出。

规划事件：

```text
workspace://changed
knowledge://index-progress
knowledge://document-stale
document://external-change
conversation://token
agent://run-updated
agent://approval-required
operations://history-changed
```

固定依赖方向：

```text
React feature
  → 窄化 Tauri Command
  → Rust application service
  → domain policy
  → repository / provider / filesystem adapter
```

## 知识索引与问答数据流

```text
文件变化
  → 文件身份与内容指纹复核
  → 格式识别和正文提取
  → 结构化切片
  → FTS5 关键词索引
  → 嵌入 Provider 生成向量
  → 事务切换活动索引版本
```

- Markdown 保存标题路径，PDF 保存页码，DOCX 保存段落序号，代码保存行号。
- 默认切片目标约 600 tokens，重叠约 80 tokens；最终值须以评测为准。
- FTS5 与向量检索分别取前 20 个候选，用 Reciprocal Rank Fusion 合并，并按相关性、文档多样性与上下文预算重排。
- 最多向回答模型提供 8 个文本块；模型只能引用当前提供的 `chunkId`。
- Rust 校验所有引用。非法引用最多发起一次受限修复，仍非法时不展示伪引用；证据不足时明确拒答。
- 向量仓储通过接口隔离实现。`sqlite-vec` 是首个设计候选，不是已接入依赖；引入前必须锁定精确版本并校验扩展版本与维度。
- 默认嵌入模型候选为 `qwen3-embedding:0.6b`，计划通过 Ollama `/api/embed` 调用；当前尚未接入、下载或评测。

## 持久化设计

规划新增表：

```text
knowledge_spaces
knowledge_roots
knowledge_documents
document_chunks
document_chunks_fts
chunk_embeddings
index_jobs
index_failures
conversations
chat_messages
message_citations
document_versions
document_edit_history
agent_runs
agent_steps
agent_tool_calls
approval_requests
```

知识文本块和向量属于用户主动建立索引后产生的本地、可重建派生数据。它们与阶段五“单次 AI 整理分析不持久化原始正文”的现行规则并存：阶段五现状不变，阶段七将通过独立、可见、可清除的知识索引授权引入持久文本块。

## 写入与 Agent 安全边界

Markdown/TXT 编辑遵循：草稿 → 差异 → Rust 校验 → `editPlanId` → 用户确认 → 版本快照 → 同目录临时文件 → 原子替换 → 历史 → 增量索引。`editPlanId` 只在 Rust 内存保存 10 分钟且一次性消费；前端只有在直接响应用户明确确认动作时才能提交，前端业务逻辑、AI 和 Agent 不得自行确认、访问计划存储或替换内容。

Agent 只通过以下首版白名单工具工作：

```text
knowledge.search
document.read
document.propose_create
document.propose_patch
file.propose_rename
file.propose_move
```

读工具只能访问当前知识空间；写工具只能产生草稿、差异或路径预览。工具结果不得向模型返回 `planId` 或 `editPlanId`。只有用户明确操作界面才能批准执行。Agent 不得依赖文件执行器、计划存储、撤销服务或数据库连接，只能调用公开工具接口。审计记录使用脱敏封闭结构化参数，关联知识空间/文档 ID、权限决策、状态迁移和审批事件，不保存完整正文、隐藏推理或一次性计划 ID。

## 错误与安全表达

- 普通错误在所属面板中显示；需要用户处理的冲突进入审批中心。
- 文件变化会使旧文本块、引用、编辑计划和相关 Agent 提案失效。
- 索引失败按文件隔离并可恢复，不修改源文件；旧版本在文件变化后立即停止参与检索。
- AI、嵌入 Provider 或向量扩展不可用时，文件浏览和手动安全操作继续工作。
- Agent 取消、超时或进程重启后不得继续旧计划或排队工具。
- 任何未授权路径、未知工具、额外参数或超限结果都必须在 Rust 边界拒绝并记录审计。

## 测试策略

- 阶段六：工作空间切换、标签恢复、选择同步、键盘与窄窗口组件测试。
- 阶段七：多根授权、链接/Junction、增量索引、中断恢复、模型变化与源文件零修改测试。
- 阶段八：检索黄金集、引用合法性、无答案拒答、定位准确性与延迟测试。
- 阶段九：外部变化、编码/BOM/换行、计划生命周期、原子替换失败和版本恢复测试。
- 阶段十：工具 Schema、权限、审批、取消、超时、重启和完整审计测试。
- 所有真实文件系统测试使用隔离临时目录，不接触用户真实数据。

## 验收门槛

- 阶段一至五既有回归继续通过，安全移动/重命名管线未被绕过。
- 1–2 万文件、10 万文本块目标规模下，索引可中断恢复且不修改源文件。
- 引用 ID 合法率 100%，Recall@5 不低于 85%，无答案正确拒答率不低于 90%。
- 10 万文本块预热混合检索 P95 不高于 2.5 秒。
- 外部文件变化不会被编辑保存覆盖；成功编辑可从版本快照恢复。
- 未经用户批准的 Agent 写操作执行数为零；未授权路径、未知工具和非法参数拦截率 100%。

这些数值是未来发布门槛，当前尚未运行知识库或 Agent 评测，不能据此声称已经达标。

## 已知限制与待决事项

- `sqlite-vec` 版本、打包方式、跨平台扩展加载和回退实现须在阶段七开始前验证。
- `qwen3-embedding:0.6b` 的向量维度、内存、吞吐与中文检索质量尚未验证。
- PDF 与 DOCX 的定位稳定性取决于提取器，首版不处理 OCR 和复杂版面语义。
- “原子替换”必须按 Windows、macOS、Linux 文件系统语义分别验证，不能只依赖单平台假设。
- 阶段十只实现知识工作 Agent，不扩展为通用电脑 Agent。
