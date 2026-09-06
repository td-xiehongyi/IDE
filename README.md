# 本地个人知识库与 Agent

由 ai-file-sorter / AI File Organizer 转型而来的 Windows 桌面应用。新定位是：以 Markdown 为核心，在一个应用中阅读、创建、编辑和连接个人笔记，让 Agent 基于知识库回答问题，并在用户确认后维护内容。

**当前状态：2026-09-06 产品方向与首版方案已确认，文档已切换到新定位；新知识库能力尚未完成。** 原工程名称、包标识和应用数据路径暂时保留，避免把文档更新误作应用迁移。

## 首版目标

- 本地文件夹就是知识库，初期面向几百篇 Markdown；主要服务个人日常使用，兼顾学习和作品展示。
- 应用内即时预览编辑，保留源码模式；手动编辑自动保存，保留可恢复历史。
- 支持 [[笔记链接]]、反向链接、搜索、重命名、移动与可恢复删除。
- 云端使用 OpenAI 兼容接口，本地连接 Ollama；用户选择模型，API 密钥进入系统凭据存储。
- Agent 在当前知识库内按需检索和读取，可限定文件夹或笔记。回答可追溯到资料，维护操作先逐文件预览、整组确认，再执行，并支持任务回退。
- 首版不做联网搜索、图片识别、其他格式的内容处理、跨设备同步或手机端。

## 当前代码事实

检查入口为 [App](src/app/App.tsx)、[Workbench](src/features/workbench/Workbench.tsx) 和 [Rust Command 注册](src-tauri/src/lib.rs)。

| 能力 | 当前证据边界 |
| --- | --- |
| 扫描、元数据索引、搜索、监听、受控移动/重命名和历史 | 已有代码与测试文件，作为复用基础；本轮未重新执行功能测试 |
| 本地 AI 文件分析 | 已有旧文件整理业务实现，不等于知识问答 |
| 工作台 | 已由 App 挂载；正文查看、Ask 和 Agent 仍显示未启用 |
| Markdown 编辑、自动保存、链接维护、版本恢复 | 新目标，不能宣称现有应用已支持 |
| 知识问答、云端接入、Agent 与任务回退 | 新目标，未完成端到端实现 |

本轮只调整文档，未启动桌面应用、未验证安装包，也未修改上述实现。旧文档中的测试与阶段状态属于历史证据。

## 文档入口

- [产品需求](docs/PRD.md)：已确认范围、用户流程与验收场景。
- [系统架构](docs/ARCHITECTURE.md)：现有基础、目标模块、数据流与待验证选型。
- [安全与恢复规则](docs/SAFETY_MODEL.md)：自动保存、Agent 审批、冲突、回收和回退。
- [开发路线图](docs/ROADMAP.md)：知识库 → 知识问答 → Agent 维护的建设顺序。
- [启动决定记录](docs/kickoff.md)：用户确认、建议与旧决定的替代关系。
- [历史资料](docs/history/README.md)：旧文件整理实现、被替代规划及本轮替换前快照。

## 开发环境

当前配置使用 Tauri 2、React、TypeScript、Rust 和 SQLite。package.json 声明 Node.js 24、pnpm 11；另需 Rust 工具链和 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。

以下命令来自当前项目配置，不代表本轮已执行通过：

```powershell
# 在仓库根目录执行
pnpm install --frozen-lockfile
pnpm dev
pnpm tauri dev

# 前端检查
pnpm check

# Rust 检查
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml

# 调试安装包
pnpm tauri build --debug
```

基础编辑与搜索应可离线使用。云端 AI 任务会发送所需相关内容；“本地知识库”不表示云端推理也在本机运行。
