> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段七设计：知识空间与知识索引核心

| 属性 | 值 |
| --- | --- |
| 状态 | 设计完成；未实现 |
| 目标版本 | `v0.2.0` |
| 最后更新 | `2026-08-23` |
| 依赖 | [阶段六工作台](./PHASE_06_WORKBENCH_UI.md)；[安全模型](./SAFETY_MODEL.md)；阶段二至三扫描、索引与监听能力 |

## 目标

- 引入项目式 `KnowledgeSpace`，让一个知识空间包含一个或多个用户明确授权的真实根目录。
- 将原文件保持为事实来源，在系统应用数据目录的 SQLite 中保存可重建文本块、关键词索引、向量和任务状态。
- 建立可取消、可恢复、逐文件隔离失败的增量索引流水线。
- 为阶段八检索提供稳定的文档身份、位置、活动索引版本与可替换向量仓储边界。

## 非目标

- 不修改、复制、移动或重命名源文件，不在授权根目录写旁车文件。
- 不支持 OCR、扫描版 PDF、XLSX、PPTX、旧版 `.doc` 或多模态索引。
- 不在本阶段实现回答生成、对话、编辑或 Agent。
- 不把 `sqlite-vec` 或 `qwen3-embedding:0.6b` 描述为当前已安装、已下载、已接入或已评测。
- 不通过符号链接、Junction、路径字符串前缀或监听事件扩展授权。

## 用户流程

```text
创建知识空间
  → 通过原生目录选择器添加一个或多个真实根目录
  → 展示预计文件范围与支持格式
  → 用户开始建立知识索引
  → 逐文件提取、切片、FTS5 写入和向量生成
  → 展示进度、失败、取消与恢复入口
  → 文件变化触发过期标记和增量重建
```

移除根目录只解除知识空间关联并清除其可重建派生索引，不删除物理文件或阶段四操作历史。清除动作必须显示数据范围并由用户确认。

## 领域模型

```ts
type KnowledgeSpace = {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
};

type KnowledgeRoot = {
  id: string;
  spaceId: string;
  canonicalPath: string;
  displayName: string;
  authorizationState: "active" | "unavailable" | "revoked";
};

type DocumentIdentity = {
  canonicalPath: string;
  entryKind: "regular_file";
  sizeBytes: number;
  modifiedAt: string;
  platformFileId?: string;
  contentFingerprint: string;
};

type IndexState =
  | "not_indexed"
  | "queued"
  | "extracting"
  | "embedding"
  | "ready"
  | "stale"
  | "failed";

type IndexJobState =
  | "queued"
  | "running"
  | "cancelling"
  | "cancelled"
  | "completed"
  | "failed";
```

`KnowledgeDocument` 使用稳定文档 ID 连接文件身份、根目录、活动索引版本和状态。重命名时只有平台文件身份与内容复核共同满足策略，才能保留文档 ID；否则创建新文档并让旧文档失效。

## 索引数据流

```text
扫描或监听发现文件版本
  → 授权、普通文件、格式和资源限制复核
  → 流式读取并计算内容指纹
  → 格式提取器产生带位置的结构化文本
  → 按结构切分为目标约 600 tokens、重叠约 80 tokens 的文本块
  → 在候选 indexVersion 写入文档块和 FTS5
  → 通过 EmbeddingProvider 生成并校验向量
  → 单个文档全部成功后事务切换 activeIndexVersion
```

- Markdown 块保留 `headingPath`。
- PDF 块保留 `pageNumber`。
- DOCX 块保留段落序号，并映射到 `characterStart/End`（可获得时）。
- 代码和配置文本保留 `lineStart/End`。
- TXT 保留行号或字符范围。
- 指纹、提取器版本、切片配置、嵌入模型名、模型摘要、向量维度和索引版本必须一同记录。

文件变化后，旧活动版本立即标为不可检索；新版本未完全成功前不暴露部分文本块。失败时源文件不受影响，旧版本只作为可清理数据保留，不重新参与检索。

## 存储与仓储接口

规划表：

```text
knowledge_spaces
knowledge_roots
knowledge_documents
document_chunks
document_chunks_fts
chunk_embeddings
index_jobs
index_failures
```

阶段七计划创建上述表并使用 SQLite FTS5；当前数据库尚未建立 `document_chunks`、`document_chunks_fts`、向量表或知识索引。实现后 `document_chunks_fts` 才会与活动文档版本关联。向量层通过仓储接口隔离：

```rust
trait VectorRepository {
    fn validate_runtime(&self, expected_dimension: usize) -> Result<(), VectorError>;
    fn replace_document_vectors(&self, version: &IndexVersion, items: &[Embedding]) -> Result<(), VectorError>;
    fn search(&self, query: &[f32], scope: &SearchScope, limit: usize) -> Result<Vec<VectorHit>, VectorError>;
    fn purge_space(&self, space_id: &KnowledgeSpaceId) -> Result<(), VectorError>;
}

trait EmbeddingProvider {
    fn descriptor(&self) -> EmbeddingModelDescriptor;
    fn embed(&self, inputs: &[String], cancellation: &CancellationToken) -> Result<Vec<Vec<f32>>, ProviderError>;
}
```

首个向量实现候选为锁定精确版本的 `sqlite-vec`。启动时必须校验扩展可加载、版本匹配、向量维度匹配和最小查询自检；失败时禁用向量检索并展示降级状态，不影响文件浏览或手动操作。

默认嵌入模型候选为 Ollama `qwen3-embedding:0.6b`，计划调用 `/api/embed`。实现前必须确认模型存在、输出维度稳定、批量上限与归一化约定；所有这些当前均尚未验证。

## 规划 Command 与事件

| 领域入口 | 作用 |
| --- | --- |
| `workspace/list`、`workspace/create`、`workspace/update` | 管理知识空间元数据 |
| `workspace/add-root`、`workspace/remove-root` | 经原生授权管理真实根目录 |
| `knowledge-index/start` | 为知识空间或选中文档创建索引任务 |
| `knowledge-index/cancel`、`knowledge-index/resume` | 管理可恢复任务 |
| `knowledge-index/status`、`knowledge-index/failures` | 查询进度与逐文件错误 |
| `knowledge-index/clear` | 经确认清除可重建文本块与向量 |

这些是领域命名，不是通用字符串路由设计。实际 Tauri Command 必须逐个注册固定结构。事件使用 `workspace://changed`、`knowledge://index-progress` 和 `knowledge://document-stale`。

## 错误与安全

- 读取前检查普通文件类型、授权范围、大小、压缩容器声明与实际资源上限；读取过程中继续执行取消和上限检查。
- 提取结束、持久化前和活动版本切换前复核文件身份与内容指纹；变化时丢弃候选版本。
- 不跟随文件或目录链接；Junction 和重解析点不得通过重新解析绕过知识根目录。
- 每个任务有有限队列与并发上限；取消必须释放并发槽位，排队项进入明确终态。
- 嵌入 Provider 只接收已授权索引任务产生的文本块；默认本地 Provider。任何未来远程 Provider 必须另行设计用户同意与数据范围展示。
- 日志不得记录原始文本块、向量内容或敏感绝对路径；可记录不含正文的文档 ID、阶段、错误码和耗时。

## 测试矩阵

- 授权：多根目录、重复根、父子根、大小写、Unicode、链接、Junction 与根失效。
- 提取：Markdown 标题、PDF 页码、DOCX 段落、代码行号、编码错误和不支持格式。
- 版本：内容不变跳过、同大小替换、同时间替换、重命名身份保留、变化中止和模型变化重建。
- 事务：FTS 成功/向量失败、进程中断、恢复、取消、逐文件失败与活动版本原子切换。
- 存储：FTS5 查询、向量维度、扩展版本、知识空间隔离和按空间清除。
- 安全：索引过程源文件零修改、无旁车、正文不进日志、未授权文本零读取。
- 性能：1–2 万文件、10 万文本块的初建、增量、恢复、数据库大小与内存峰值。

## 验收门槛

- 用户可建立含多个明确授权根目录的知识空间，且空间间检索数据严格隔离。
- 1–2 万文件、10 万文本块目标规模下可完成索引；中断、取消和重启后可恢复。
- 指纹未变化的文件不会重复提取或嵌入；变化文件的旧版本立即停止参与检索。
- 单文档新索引只有在文本块、FTS5 和向量全部成功后才切换为活动版本。
- 索引、清除和恢复均不修改源文件或阶段四操作历史。
- 向量扩展或模型不可用时状态明确，文件管理与 FTS5 降级能力仍可用。

## 已知限制

- 600/80 tokens 是初始参数，必须依据中文、代码和长文档评测调整。
- FTS5 分词对中文的质量需实测，可能需要受控 tokenizer 方案。
- `sqlite-vec` 为 pre-v1 候选，版本与打包风险尚未消除。
- `qwen3-embedding:0.6b` 尚未接入或评测，不能作为当前能力说明。
