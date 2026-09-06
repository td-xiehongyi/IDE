# 第一阶段真实文件与保存流程 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户能够在当前 Windows 桌面应用中安全打开本地知识库、编辑 Markdown、自动保存、处理外部冲突，并查看和恢复持久历史版本。

**Architecture:** React 维护每篇打开笔记的编辑缓冲和保存状态，所有真实文件读取、校验、替换、历史持久化与恢复均通过窄化的 Tauri 命令进入 Rust。保存使用内容哈希做乐观并发检查，同一笔记串行写入；SQLite 保存版本元数据与正文快照，搜索索引保持可重建并与恢复数据分离。

**Tech Stack:** React 19、TypeScript 7、CodeMirror 6 候选、Tauri 2、Rust、SQLite/rusqlite、Vitest、Cargo integration tests、Windows 隔离目录验收。

**Spec:** `docs/FIRST_STAGE_DESIGN.md`

## Global Constraints

- 原始 Markdown 文件是正式内容，应用数据区只保存索引、版本快照和恢复元数据。
- 第一阶段只读取和编辑当前知识库内的普通 `.md` 文件。
- 停止输入 1 秒后自动保存；中文输入组合期间不启动保存计时。
- 切换、关闭、正常退出和持续编辑每 5 分钟，只为有变化且已保存的正文创建检查点。
- 保存失败或外部修改不能覆盖任意一方，也不能把失败显示成成功。
- 历史默认长期保留，由用户显式清理；索引重建不得删除历史。
- 恢复旧版本必须先展示影响并确认，恢复结果成为新版本。
- 本计划不接入 AI、Agent、联网搜索、图片识别、公式渲染、Mermaid 渲染、同步或移动端。
- 保留当前未提交工作；每个任务只提交该任务明确列出的文件。

---

## Planned File Structure

- `src-tauri/src/models/document.rs`：文档快照、保存、历史和恢复命令的数据结构。
- `src-tauri/src/services/document_service.rs`：Markdown 读取、基线复核、保存与恢复编排。
- `src-tauri/src/services/safe_replace.rs`：同目录临时文件和 Windows 替换协议，隔离平台细节。
- `src-tauri/src/storage/document_repository.rs`：版本记录和历史查询。
- `src-tauri/src/storage/migrations/006_document_history.sql`：追加历史表，不修改旧迁移。
- `src-tauri/src/commands/documents.rs`：窄化的 Tauri 文档命令。
- `src-tauri/tests/document_read.rs`：范围、格式、编码和读取测试。
- `src-tauri/tests/document_save.rs`：保存、失败和并发基线测试。
- `src-tauri/tests/document_history.rs`：检查点、恢复和索引隔离测试。
- `src/types/documents.ts`：与 Rust 序列化字段一致的前端类型。
- `src/lib/documents-api.ts`：文档命令和事件的唯一前端入口。
- `src/features/workbench/document-session.ts`：打开标签、草稿、基线和保存状态的纯状态转换。
- `src/features/workbench/useDocumentSaving.ts`：防抖、每篇串行保存和生命周期阻断。
- `src/features/workbench/DocumentEditor.tsx`：即时预览与源码模式共享同一文本状态。
- `src/features/workbench/DocumentHistoryPanel.tsx`：历史列表、版本内容和恢复确认。
- `src/features/workbench/DocumentConflictPanel.tsx`：本地草稿与磁盘版本处理。
- 对应 `*.test.ts` / `*.test.tsx`：验证纯状态、组件行为和 API 参数。

### Task 1: 验证编辑器不会改写 Markdown 原文

**Files:**
- Create: `src/features/workbench/editor-roundtrip.test.ts`
- Create: `src/features/workbench/DocumentEditor.tsx`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`

**Interfaces:**
- Consumes: `value: string`、`mode: "live" | "source"`、`onChange(next: string)`。
- Produces: `DocumentEditorProps` 与一个不经解析再序列化的编辑器组件。

- [ ] **Step 1: 固定真实 Markdown 往返样本**

在测试中写入包含中文输入、标题、列表、表格、代码块、wiki 链接、普通 Markdown 链接、公式源码和 Mermaid 代码块的字符串；切换 `live → source → live` 后断言 `onChange` 未被调用且原字符串逐字节相同。

- [ ] **Step 2: 运行测试确认当前缺少编辑器组件**

Run: `pnpm test -- src/features/workbench/editor-roundtrip.test.ts`

Expected: FAIL，原因是 `DocumentEditor` 尚不存在。

- [ ] **Step 3: 安装并实现最小 CodeMirror 6 候选**

安装 `@codemirror/state`、`@codemirror/view`、`@codemirror/lang-markdown`；组件始终使用同一 `EditorState`，即时预览只改变 decorations，源码模式移除 decorations，不把解析结果写回正文。

- [ ] **Step 4: 验证中文组合与模式切换**

补充 `compositionstart`、输入、`compositionend` 测试，断言组合期间不触发外部保存信号，结束后只产生一次最终文本更新。

- [ ] **Step 5: 运行前端验证**

Run: `pnpm test -- src/features/workbench/editor-roundtrip.test.ts`

Expected: PASS。

Run: `pnpm typecheck`

Expected: PASS。

- [ ] **Step 6: 提交编辑器候选**

```powershell
git add package.json pnpm-lock.yaml src/features/workbench/DocumentEditor.tsx src/features/workbench/editor-roundtrip.test.ts
git commit -m "feat: add markdown editor roundtrip foundation"
```

### Task 2: 定义文档命令类型和根目录边界

**Files:**
- Create: `src-tauri/src/models/document.rs`
- Create: `src-tauri/src/commands/documents.rs`
- Create: `src-tauri/tests/document_read.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: 当前 watcher 根目录、`path_policy`、现有 `file_identity`。
- Produces: `read_markdown_document(root_path, relative_path) -> DocumentSnapshot`。

- [ ] **Step 1: 写读取边界测试**

覆盖当前根目录内 UTF-8 `.md` 成功、`..` 越界、绝对外部路径、符号链接、目录、非 Markdown 和无效 UTF-8 失败。成功结果必须包含 `relative_path`、`text`、`size`、`modified_ms`、`file_identity`、`content_hash`。

- [ ] **Step 2: 运行测试确认命令缺失**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_read`

Expected: FAIL，原因是文档模型或读取服务尚未定义。

- [ ] **Step 3: 实现窄化读取命令**

命令先确认 `root_path` 等于 watcher 当前根目录，再由 `path_policy` 解析相对路径；拒绝符号链接和非普通文件，仅允许扩展名大小写不敏感的 `.md`，以 UTF-8 读取并计算 SHA-256 内容哈希。

- [ ] **Step 4: 注册命令并运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_read`

Expected: PASS。

- [ ] **Step 5: 提交读取边界**

```powershell
git add src-tauri/src/models/document.rs src-tauri/src/models/mod.rs src-tauri/src/commands/documents.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/tests/document_read.rs
git commit -m "feat: add scoped markdown document reads"
```

### Task 3: 建立持久版本仓储

**Files:**
- Create: `src-tauri/src/storage/migrations/006_document_history.sql`
- Create: `src-tauri/src/storage/document_repository.rs`
- Create: `src-tauri/tests/document_history.rs`
- Modify: `src-tauri/src/storage/mod.rs`
- Modify: `src-tauri/src/storage/database.rs`

**Interfaces:**
- Consumes: `scan_roots.id` 和 `DocumentSnapshot`。
- Produces: `insert_version`、`list_versions`、`read_version`、`delete_versions`。

- [ ] **Step 1: 写迁移和仓储失败测试**

测试从现有版本 5 数据库升级后旧表与旧操作历史仍存在；相同根目录、路径和内容哈希不重复插入；不同知识库的历史互不混合；按创建时间倒序分页。

- [ ] **Step 2: 运行测试确认迁移缺失**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_history`

Expected: FAIL，原因是版本 6 迁移或仓储不存在。

- [ ] **Step 3: 添加只追加迁移**

创建 `document_versions`，字段为 `id`、`root_id`、`relative_path`、`content_hash`、`content`、`size`、`modified_ms`、`file_identity`、`reason`、`source`、`created_at`；添加 `(root_id, relative_path, created_at DESC, id DESC)` 索引和避免同路径连续重复内容的仓储检查。

- [ ] **Step 4: 实现仓储并运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_history`

Expected: PASS，并确认 `rebuild_index` 只清理可重建索引，不删除 `document_versions`。

- [ ] **Step 5: 提交版本仓储**

```powershell
git add src-tauri/src/storage/migrations/006_document_history.sql src-tauri/src/storage/document_repository.rs src-tauri/src/storage/mod.rs src-tauri/src/storage/database.rs src-tauri/tests/document_history.rs
git commit -m "feat: persist markdown document history"
```

### Task 4: 实现带基线校验的安全保存

**Files:**
- Create: `src-tauri/src/services/document_service.rs`
- Create: `src-tauri/src/services/safe_replace.rs`
- Create: `src-tauri/tests/document_save.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/commands/documents.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `SaveDocumentRequest { root_path, relative_path, expected_hash, text, edit_generation, checkpoint_reason }`。
- Produces: `SaveResult { status, content_hash, modified_ms, file_identity, edit_generation, version_id }`，其中 `status` 为 `saved | conflict | failed`。

- [ ] **Step 1: 写保存协议测试**

覆盖基线一致保存成功、基线不一致返回 conflict 且两份内容都保留、权限失败不显示 saved、临时文件失败不破坏原文、重复请求不产生重复历史、保存结果回传原 generation。

- [ ] **Step 2: 运行测试确认保存服务缺失**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_save`

Expected: FAIL。

- [ ] **Step 3: 实现平台隔离的替换接口**

写入同目录唯一临时文件，刷新文件内容并复核目标哈希，再调用 Windows 替换实现；失败后检查目标与临时文件的实际状态并返回可诊断结果，不根据 API 错误码假定原文件未变化。

- [ ] **Step 4: 在成功写入前后记录恢复信息**

首次改写前持久保存原文；只有实际成功且需要检查点时写入新历史。历史持久化失败时阻止会削弱恢复能力的写入，并保留调用方正文。

- [ ] **Step 5: 运行保存和历史测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_save --test document_history`

Expected: PASS。

- [ ] **Step 6: 提交安全保存**

```powershell
git add src-tauri/src/services/document_service.rs src-tauri/src/services/safe_replace.rs src-tauri/src/services/mod.rs src-tauri/src/commands/documents.rs src-tauri/src/lib.rs src-tauri/tests/document_save.rs
git commit -m "feat: save markdown with conflict protection"
```

### Task 5: 建立前端文档会话和串行自动保存

**Files:**
- Create: `src/types/documents.ts`
- Create: `src/lib/documents-api.ts`
- Create: `src/features/workbench/useDocumentSaving.ts`
- Create: `src/features/workbench/useDocumentSaving.test.ts`
- Modify: `src/features/workbench/document-session.ts`
- Modify: `src/features/workbench/document-session.test.ts`

**Interfaces:**
- Consumes: Task 2/4 的读取和保存命令。
- Produces: `DocumentTab` 的 `draftText`、`baseSnapshot`、`editGeneration`、`saveState`，以及 `scheduleSave`、`flushSave`、`retrySave`。

- [ ] **Step 1: 写纯状态转换测试**

验证 `clean → dirty → saving → clean`、保存中再次输入仍为 dirty、旧 generation 成功不清除新草稿、失败保留草稿、conflict 阻止后续自动保存、不同标签的保存队列互不阻塞。

- [ ] **Step 2: 运行测试确认状态模型不完整**

Run: `pnpm test -- src/features/workbench/document-session.test.ts src/features/workbench/useDocumentSaving.test.ts`

Expected: FAIL。

- [ ] **Step 3: 实现每篇笔记一个串行队列**

停止输入 1000ms 后提交；`compositionstart` 取消计时，`compositionend` 重新调度；Ctrl+S 和生命周期操作调用 `flushSave`。一个请求进行中时仅记录最新待提交 generation，请求完成后最多继续一次最新保存。

- [ ] **Step 4: 运行前端测试**

Run: `pnpm test -- src/features/workbench/document-session.test.ts src/features/workbench/useDocumentSaving.test.ts`

Expected: PASS。

- [ ] **Step 5: 提交文档会话**

```powershell
git add src/types/documents.ts src/lib/documents-api.ts src/features/workbench/document-session.ts src/features/workbench/document-session.test.ts src/features/workbench/useDocumentSaving.ts src/features/workbench/useDocumentSaving.test.ts
git commit -m "feat: add serialized markdown autosave sessions"
```

### Task 6: 接入工作台、标签切换与关闭保护

**Files:**
- Modify: `src/features/workbench/Workbench.tsx`
- Modify: `src/features/workbench/Workbench.test.tsx`
- Modify: `src/features/workbench/DocumentEditor.tsx`
- Modify: `src/app/styles.css`

**Interfaces:**
- Consumes: `readMarkdownDocument`、`useDocumentSaving`、`DocumentEditor`。
- Produces: 真实笔记打开、即时预览/源码切换、保存状态和受保护的标签切换/关闭。

- [ ] **Step 1: 写用户流程测试**

验证点击 `.md` 后读取正文；非 Markdown 保持只读文件列表行为；编辑后显示待保存；1 秒后显示保存中和已保存；保存失败时切换或关闭被暂停并显示重试、另存副本、取消。

- [ ] **Step 2: 运行测试确认仍是空白文档会话**

Run: `pnpm test -- src/features/workbench/Workbench.test.tsx`

Expected: FAIL。

- [ ] **Step 3: 接入编辑器和保存状态**

用 draft-15 确认的标签、中间编辑区和工具栏样式替换正文占位；Agent 仍显示未接入提示。切换模式只改变编辑器显示，不创建保存或改写正文。

- [ ] **Step 4: 接入切换、关闭、切库和退出阻断**

这些动作先 `flushSave`；saved 后继续，failed/conflict 后停留在原笔记并显示处理面板。浏览器 `beforeunload` 只作为补充提示，不能替代 Tauri 关闭流程。

- [ ] **Step 5: 运行工作台测试和构建**

Run: `pnpm test -- src/features/workbench/Workbench.test.tsx`

Expected: PASS。

Run: `pnpm build`

Expected: PASS。

- [ ] **Step 6: 提交工作台接入**

```powershell
git add src/features/workbench/Workbench.tsx src/features/workbench/Workbench.test.tsx src/features/workbench/DocumentEditor.tsx src/app/styles.css
git commit -m "feat: connect markdown editing to the workbench"
```

### Task 7: 实现外部修改冲突处理

**Files:**
- Create: `src/features/workbench/DocumentConflictPanel.tsx`
- Create: `src/features/workbench/DocumentConflictPanel.test.tsx`
- Modify: `src/lib/documents-api.ts`
- Modify: `src/features/workbench/Workbench.tsx`
- Modify: `src-tauri/src/commands/documents.rs`

**Interfaces:**
- Consumes: watcher 变化事件、当前 `baseSnapshot` 与 `draftText`。
- Produces: `DocumentChangedEvent` 和采用磁盘、保留本地副本、重新预览后覆盖三条显式路径。

- [ ] **Step 1: 写冲突面板测试**

无本地变化时自动刷新并提示；有本地变化时不更新编辑器正文，显示本地草稿和磁盘正文；采用磁盘内容前要求确认；本地覆盖再次校验哈希；外部删除不由自动保存重新创建。

- [ ] **Step 2: 运行测试确认冲突面板缺失**

Run: `pnpm test -- src/features/workbench/DocumentConflictPanel.test.tsx src/features/workbench/Workbench.test.tsx`

Expected: FAIL。

- [ ] **Step 3: 发布窄化的文档变化事件**

事件只发送当前根目录内的相对路径、变化类型和可用的新哈希；前端只标记对应标签，不把事件正文直接写入草稿。

- [ ] **Step 4: 实现三条处理路径并验证**

Run: `pnpm test -- src/features/workbench/DocumentConflictPanel.test.tsx src/features/workbench/Workbench.test.tsx`

Expected: PASS。

- [ ] **Step 5: 提交冲突处理**

```powershell
git add src/features/workbench/DocumentConflictPanel.tsx src/features/workbench/DocumentConflictPanel.test.tsx src/lib/documents-api.ts src/features/workbench/Workbench.tsx src-tauri/src/commands/documents.rs
git commit -m "feat: protect drafts from external file changes"
```

### Task 8: 实现历史查看、差异预览和恢复

**Files:**
- Create: `src/features/workbench/DocumentHistoryPanel.tsx`
- Create: `src/features/workbench/DocumentHistoryPanel.test.tsx`
- Modify: `src/lib/documents-api.ts`
- Modify: `src-tauri/src/commands/documents.rs`
- Modify: `src-tauri/src/services/document_service.rs`
- Modify: `src/features/workbench/Workbench.tsx`

**Interfaces:**
- Consumes: `get_document_history`、`read_document_version`、`restore_document_version`。
- Produces: 当前笔记的历史时间线、完整内容/差异预览和确认恢复。

- [ ] **Step 1: 写历史与恢复测试**

验证历史只显示当前笔记；选择版本后可查看当前与目标；未确认不写文件；确认恢复前校验当前哈希；冲突时建议恢复副本；成功恢复产生新版本且较新历史仍可查看。

- [ ] **Step 2: 运行测试确认历史面板缺失**

Run: `pnpm test -- src/features/workbench/DocumentHistoryPanel.test.tsx`

Expected: FAIL。

- [ ] **Step 3: 实现历史命令和面板**

左下角历史入口绑定当前活动标签；无活动笔记时禁用。版本列表分页加载，差异和完整内容按需读取，恢复按钮在第二次明确确认后调用 Rust。

- [ ] **Step 4: 运行前后端恢复验证**

Run: `pnpm test -- src/features/workbench/DocumentHistoryPanel.test.tsx src/features/workbench/Workbench.test.tsx`

Expected: PASS。

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test document_history --test document_save`

Expected: PASS。

- [ ] **Step 5: 提交历史恢复**

```powershell
git add src/features/workbench/DocumentHistoryPanel.tsx src/features/workbench/DocumentHistoryPanel.test.tsx src/lib/documents-api.ts src-tauri/src/commands/documents.rs src-tauri/src/services/document_service.rs src/features/workbench/Workbench.tsx
git commit -m "feat: add document history preview and restore"
```

### Task 9: 接入检查点、重启恢复和历史清理

**Files:**
- Modify: `src/features/workbench/useDocumentSaving.ts`
- Modify: `src/features/workbench/useDocumentSaving.test.ts`
- Modify: `src/features/workbench/DocumentHistoryPanel.tsx`
- Modify: `src/features/workbench/DocumentHistoryPanel.test.tsx`
- Modify: `src-tauri/src/commands/documents.rs`
- Modify: `src-tauri/src/storage/document_repository.rs`

**Interfaces:**
- Consumes: 已成功保存的最新 `DocumentSnapshot`。
- Produces: 五分钟检查点、切换/关闭检查点、最近知识库恢复和显式历史清理。

- [ ] **Step 1: 写检查点与清理测试**

验证未变化不生成版本；保存失败不生成已完成版本；持续编辑满 5 分钟只保存最新成功正文；普通清理不删除当前文件、回收内容、执行中记录或任务回退依赖；显示实际可释放字节。

- [ ] **Step 2: 运行测试确认行为缺失**

Run: `pnpm test -- src/features/workbench/useDocumentSaving.test.ts src/features/workbench/DocumentHistoryPanel.test.tsx`

Expected: FAIL。

- [ ] **Step 3: 实现检查点调度和清理预览**

检查点原因固定为 `first_edit`、`switch`、`close`、`exit`、`five_minutes`、`restore_before`；清理先返回版本数、独占字节和受影响记录，第二次命令携带预览令牌执行，令牌过期或依赖变化时拒绝。

- [ ] **Step 4: 运行完整自动检查**

Run: `pnpm check`

Expected: PASS。

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS。

- [ ] **Step 5: 提交生命周期与清理**

```powershell
git add src/features/workbench/useDocumentSaving.ts src/features/workbench/useDocumentSaving.test.ts src/features/workbench/DocumentHistoryPanel.tsx src/features/workbench/DocumentHistoryPanel.test.tsx src-tauri/src/commands/documents.rs src-tauri/src/storage/document_repository.rs
git commit -m "feat: add document checkpoints and history cleanup"
```

### Task 10: Windows 隔离目录验收

**Files:**
- Create: `docs/previews/phase-acceptance/phase-01/REAL_FILES_AND_SAVING.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/FIRST_STAGE_DESIGN.md`

**Interfaces:**
- Consumes: Tasks 1–9 的完整应用路径。
- Produces: 真实 Windows 运行证据、已知限制和阶段状态。

- [ ] **Step 1: 建立一次性验收知识库**

在明确的测试目录复制 Markdown 样本，记录文件数量、总字节、最长笔记和中文/表格/代码/公式/Mermaid 样本；不使用用户唯一资料做破坏测试。

- [ ] **Step 2: 验证日常完整流程**

打开测试知识库、编辑中文、切换两种模式、等待自动保存、关闭并重启、打开历史并恢复，逐项记录屏幕结果和磁盘哈希。

- [ ] **Step 3: 验证故障和竞争**

分别测试外部编辑、目标被占用、只读权限、临时文件失败模拟、应用在各保存断点中止后重启；确认原文、草稿、历史和界面状态与实际磁盘一致。

- [ ] **Step 4: 验证索引隔离**

记录历史版本数量，重建搜索索引，再次读取历史并恢复一个版本，确认版本数据未被清理。

- [ ] **Step 5: 更新阶段证据**

只把实际通过的场景标为通过；编辑器自动测试、浏览器预览、Tauri 运行、Windows 故障验证和用户日常确认分别记录。

- [ ] **Step 6: 提交验收记录**

```powershell
git add docs/previews/phase-acceptance/phase-01/REAL_FILES_AND_SAVING.md docs/ROADMAP.md docs/FIRST_STAGE_DESIGN.md
git commit -m "docs: record phase one file saving acceptance"
```

## Self-Review Result

- Spec coverage: 已覆盖真实文件范围、UTF-8 Markdown、编辑器原文往返、自动保存、串行请求、外部冲突、检查点、历史、恢复、清理、索引隔离和 Windows 验收。
- Explicitly deferred: AI、Agent、公式/Mermaid 渲染、图片、联网、非 Markdown、同步和手机端与阶段规范一致。
- Placeholder scan: 实施步骤均包含具体目标、文件、命令和预期结果；需要实验确认的行为安排在 Task 1、4 和 10 的明确验证门槛中。
- Type consistency: `DocumentSnapshot`、`SaveDocumentRequest`、`SaveResult` 和 `DocumentVersion` 从 Rust 模型经 `documents-api.ts` 到前端会话保持同名语义。
