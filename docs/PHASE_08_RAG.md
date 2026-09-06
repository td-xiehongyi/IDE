> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段八设计：RAG 问答与可验证引用

| 属性 | 值 |
| --- | --- |
| 状态 | 设计完成；未实现 |
| 目标版本 | `v0.2.0` |
| 最后更新 | `2026-08-23` |
| 依赖 | [阶段七知识核心](./PHASE_07_KNOWLEDGE_CORE.md)；[知识评测规范](./KNOWLEDGE_EVALUATION.md)；本地 Ollama Provider 边界 |

## 目标

- 支持在当前文档、选中文档或整个知识空间内进行有依据问答。
- 使用 FTS5 与向量检索混合召回，并为每条回答提供可点击、可校验、可定位的引用。
- 对证据不足、索引过期、模型非法引用和 Provider 故障提供真实状态，不生成伪引用。
- 将对话、消息和引用元数据保存在本地 SQLite，不保存模型隐藏推理过程。

## 非目标

- 不提供开放网络搜索、远程资料抓取或知识空间外文件访问。
- 不让回答直接触发移动、重命名、编辑或任何 Agent 写入。
- 不把模型声称的文件名、页码或 `chunkId` 当作可信引用。
- 不保存或展示模型隐藏推理过程。
- 不支持 OCR、图片、音视频或复杂页面多模态问答。

## 用户流程

```text
用户打开文档或选择文件
  → 明确选择 ContextScope（默认 CurrentDocument）
  → 输入问题
  → Rust 校验知识空间、索引状态与范围
  → FTS5 + 向量候选召回并融合
  → 最多 8 个带 chunkId 和位置的证据块发送给本地模型
  → Rust 校验结构化回答中的全部引用
  → 展示回答与引用卡片
  → 点击引用，在中间文档区定位并高亮
```

扩大到 `SelectedDocuments` 或 `KnowledgeSpace` 必须由用户主动选择。切换知识空间后不得静默沿用旧空间范围或引用。

## 类型与接口

```ts
type ContextScope =
  | { kind: "CurrentDocument"; documentId: string }
  | { kind: "SelectedDocuments"; documentIds: string[] }
  | { kind: "KnowledgeSpace"; spaceId: string };

type KnowledgeQuery = {
  spaceId: string;
  question: string;
  scope: ContextScope;
  conversationId?: string;
};

type RetrievedChunk = {
  chunkId: string;
  documentId: string;
  indexVersion: string;
  displayName: string;
  location: DocumentLocation;
  excerpt: string;
  score: number;
};

type Citation = {
  chunkId: string;
  documentId: string;
  displayName: string;
  location: DocumentLocation;
  excerpt: string;
};

type GroundedAnswer = {
  answer: string;
  citations: Citation[];
  evidenceState: "grounded" | "insufficient";
};
```

`DocumentLocation` 由阶段六统一定义，允许 `headingPath`、`pageNumber`、行范围和字符范围组合。

规划领域入口：`retrieval/query`、`conversation/create`、`conversation/list`、`conversation/get`、`conversation/delete`、`conversation/cancel`。流式回答使用 `conversation://token`，但只有最终通过引用校验的消息才能标记为完成并持久化为可引用答案。

## 检索与回答数据流

1. 校验问题非空、空间存在、范围属于当前空间、相关文档活动索引可用。
2. 将查询分别提交给 FTS5 与嵌入 Provider，各取前 20 个候选。
3. 使用 Reciprocal Rank Fusion 合并排名；初始常量 `k = 60`，后续只能通过锁定评测集调整。
4. 按相关性、文档多样性、重复内容和上下文预算重排。
5. 最多选择 8 个文本块，并为每块分配当前请求内不可伪造的 `chunkId`。
6. 模型返回封闭结构：回答、引用 `chunkId` 列表和证据充分性判断。
7. Rust 验证引用仅来自本次上下文、活动索引版本仍有效，且答案需要引用时引用集合非空。
8. 存在非法引用时，最多使用同一证据发起一次受限修复请求；仍非法则拒绝展示该答案。
9. 无相关候选、得分低于评测阈值或模型判定证据不足时返回明确拒答。

回答模型看不到绝对路径，只接收显示名、结构化位置、文本块和请求内 `chunkId`。最终引用中的路径与定位由 Rust 使用文档 ID 重新解析。

## 持久化

规划表：

```text
conversations
chat_messages
message_citations
```

- 消息保存用户问题、最终回答、证据状态、模型/提示词版本、范围与时间。
- 引用保存消息 ID、文档 ID、活动索引版本、文本块 ID、位置和必要的短摘录。
- 不保存模型隐藏推理过程、Provider 原始响应或超出引用所需的大段正文。
- 文档变化后，历史引用显示“来源已变化”；可以保留历史摘录用于审计，但不得把它当作当前有效证据。
- 用户可按知识空间清除会话与引用，不影响源文件和文件操作历史。

## 错误与安全

- `scope_outside_space`：范围包含非当前知识空间文档，拒绝请求。
- `index_not_ready` / `document_stale`：相关活动索引不可用，提示先完成索引。
- `embedding_unavailable`：向量侧不可用；若产品允许 FTS5 降级，界面必须明确标记检索模式和质量变化。
- `insufficient_evidence`：证据不足，返回拒答而不是补写常识。
- `invalid_citation`：模型引用不在本次候选中；一次修复失败后不展示回答。
- `source_changed`：生成期间文档版本变化，取消回答或使其失效。
- `provider_error` / `cancelled`：不保存半成品为完成消息，不影响浏览或手动操作。

来自文档的文本必须视为不可信数据。提示注入内容不能改变工具权限、上下文范围、系统指令或引用规则；本阶段回答流程没有写工具。

## 测试矩阵

- 范围：三种 `ContextScope`、空选择、跨空间文档、切换空间和默认范围。
- 召回：FTS5、向量、RRF、重复块、文档多样性、中文查询、代码查询和专有名词。
- 引用：合法 ID、未知 ID、其他请求 ID、旧索引版本、零引用答案、一次修复和点击定位。
- 拒答：无答案、相似但不支持、冲突证据、索引过期和低分候选。
- 持久化：重启恢复、按空间隔离、删除会话、来源变化标记和无隐藏推理字段。
- 故障：Ollama 不可用、嵌入失败、取消、超时、流中断和文件生成中变化。
- 性能：10 万文本块预热检索延迟、端到端首 token、内存与上下文预算。

## 验收门槛

- 引用 `chunkId` 合法率 100%，任何非法引用都不会作为有效引用展示。
- 每条引用可打开正确文档并定位到对应标题、页码、行或字符范围。
- 锁定黄金集 Recall@5 不低于 85%。
- 无答案问题正确拒答率不低于 90%。
- 10 万文本块下，预热混合检索 P95 不高于 2.5 秒。
- 对话严格按知识空间隔离，本地持久化不包含模型隐藏推理过程。
- AI、嵌入或检索失败不影响文件浏览和阶段四手动安全操作。

以上是未来验收门槛，当前尚未实现或运行相关评测。

## 已知限制

- RRF 常量、阈值、块数量和上下文预算必须通过黄金集锁定，不能凭单个示例优化。
- 中文 FTS5 召回与嵌入模型质量尚未验证。
- PDF/DOCX 的引用定位受文本提取稳定性限制；扫描版 PDF 不在范围内。
- 历史引用在源文件变化后只能用于审计，不能保证与当前内容一致。
