> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 阶段五开发指南：本地内容分析与 AI 整理建议

| 属性 | 值 |
| --- | --- |
| 状态 | 内部开发版；待修复、安全复核与真实黄金集发布验收 |
| 默认 Provider | 本地 Ollama |
| 默认模型 | `qwen2.5:7b` |
| 提示词版本 | `phase5-v1` |

## 能力与边界

阶段五只在用户勾选文件并点击“分析所选文件”后读取正文。首版支持 UTF-8、带 BOM 的 UTF-16 TXT/MD、文本型 PDF、DOCX，以及常见编程语言和配置文件；扫描版 PDF、加密或损坏文档、格式伪装、空文本和超过 100,000 字符的文档返回逐文件失败。代码文件按纯文本读取，不执行代码或进行语法解析。

代码与配置格式包括 C/C++、C#、Java/Kotlin、Go、Rust、Python、JavaScript/TypeScript、PHP、Ruby、Swift、Dart、Lua、R、Shell、PowerShell、SQL、HTML/CSS 以及 JSON、YAML、TOML、XML、INI 等；常见特殊文件名包括 `Dockerfile`、`Makefile` 和 `CMakeLists.txt`。旧版二进制 `.doc` 仍不支持。

原始正文只存在于 Rust 分析任务内存。SQLite 迁移 `003_ai_analysis.sql` 保存分类和派生结果，不存在正文、原始内容或分段文本字段。Ollama 适配器只接受 `http://localhost`、`127.0.0.1` 或 `::1` 环回地址，当前没有远程 Provider 或正文外发入口。

分析前必须同时满足：本地模型可用、用户在文件列表勾选至少一个支持文件、且至少有一个已启用分类。界面会说明未满足的条件；后端也会拒绝没有启用分类的请求。

### Windows、WSL2 与模型边界

- Windows 桌面应用通过 Rust Provider 调用 Windows Ollama 的 `http://127.0.0.1:11434`；前端不直接调用模型服务。
- WSL2 Ubuntu 只用于准备数据、执行 LLaMA-Factory QLoRA、合并权重和转换 GGUF，不运行第二套 Ollama。
- Windows Ollama 已下载的 `qwen2.5:7b` 是量化推理模型，不能作为 QLoRA 训练源。
- 训练时需要在 WSL2 单独下载 Hugging Face 格式的 `Qwen/Qwen2.5-7B-Instruct`，训练完成后再将最终 GGUF 导入 Windows Ollama。
- 训练模型使用独立名称，例如 `qwen2.5-file-organizer:7b-q4km-v1`，不得覆盖基础模型 `qwen2.5:7b`。

完整步骤见[本地模型训练指南](./LOCAL_MODEL_TRAINING_GUIDE.md)。指南完成不代表数据已经准备、模型已经训练或发布验收已经通过。

## 当前待完成项

- 修复并验证取消时的并发槽位释放，以及排队任务取消后的终态进度通知。
- 在读取前限制文件与压缩容器资源；持续补强根目录切换、分析期间文件变化和操作执行前的复核。
- 将“采用建议”的持久化状态延后到阶段四预览成功之后。
- 以 80–120 份真实非敏感文件完成黄金集评测与人工验收；在此之前不得标记为可发布。
- 远程 Provider、OCR、XLSX、PPTX、旧版 DOC、多模态理解和 QLoRA 训练不属于首版。

## 数据流与接口

```text
用户勾选文件
  → start_analysis_batch
  → Rust 格式识别、正文提取、内容指纹
  → 短文档直接分析；长文档分段分析后汇总
  → JSON Schema + Rust 封闭结构与分类/文件名校验
  → ai_analysis_results（仅派生数据）
  → 用户编辑并接受或拒绝
  → 携带内容指纹的 AiRename / AiOrganize 草案
  → 阶段四根据合法分类标签派生根目录下的单层目标目录
  → From/To 预览、确认、执行前指纹复核并安全创建或复用目录
```

前端只调用以下窄化 Command：

- `get_ai_provider_status`：检查 Ollama 与指定模型是否可用。
- `save_ai_categories` / `get_ai_categories`：管理稳定分类标签到授权根目录下单层目标目录的映射；配置和分析时目标目录可以尚不存在。
- `start_analysis_batch` / `get_analysis_batch` / `cancel_analysis_batch`：管理单并发后台批次。
- `get_analysis_results`：读取派生结果，并在读取时重新检查待审查结果是否过期。
- `review_analysis_result`：接受或拒绝建议；接受只返回操作草案，不执行文件操作。

批次通过 `ai://analysis-progress` 发送进度。一个批次最多 100 个文件；本地模型默认串行调用，避免 8GB 显存并发抖动。

## 模型与输出

Ollama `/api/chat` 请求固定 `stream: false`、`temperature: 0.1`、`num_predict: 1024`，并携带禁止额外字段的 JSON Schema。Rust 再次反序列化并校验：

- 摘要、理由和关键词非空，关键词不得重复；
- 文件名不能包含路径或非法字符，扩展名必须保持不变；
- 分类 ID 必须存在且启用，或为 `null`；
- 置信度必须位于 `[0, 1]`；
- 任何额外字段、绝对路径、未知分类或由模型提出的目录创建意图都会被拒绝。

正文超过 8,000 字符时按 400 字符重叠分段。每段先生成结构化分析，最后以这些派生结果执行一次总汇；原始分段不会持久化。

扫描、监听和 AI 分析都不会创建目录或移动文件。模型只输出分类 ID，不能输出路径，也不能要求创建目录；应用根据经过 Rust 校验的分类标签确定 `<授权根目录>/<单层分类目录>`，并且只有用户接受建议、通过 From/To 预览并确认执行后，才安全创建或复用目标目录。应用层可兼容把旧占位 ID `category_2` 与安全名称 `study` 解析为 `study` 目录，但训练集、验证集和黄金集必须统一使用语义化标签 `study`。

## 评测与训练门槛

准备 80–120 份真实非敏感文档，按格式、长度、分类和“无法分类”分层；约 60% 用于提示词开发，40% 锁定测试。每行 JSONL 使用稳定 `id`。同一次对比必须固定分类模板 ID、模板版本和标签体系版本；人工审查后的预测还需记录模型、量化版本、提示词版本、上述分类版本、文件名可接受性、摘要 1–5 分和延迟。旧占位标签统一为语义化标签后必须重新建立基线，不能把标签空间修正当作模型能力提升。

```powershell
pnpm evaluate:ai docs/evaluation/gold.example.jsonl docs/evaluation/reviewed-predictions.example.jsonl
```

脚本输出 JSON 报告；未达到以下任一门槛时退出码为 1：Schema 有效率 99%、危险输出拦截率 100%、分类准确率 85%、文件名可接受率 75%、摘要均分 4/5、短文档 P95 30 秒。

升级顺序固定为：程序约束修正 → 提示词优化 → 少量高质量示例 → 独立训练集 QLoRA。锁定测试集不得用于提示词选择或训练。

本次标签与目录规则调整不改变 `qwen2.5:7b`、`phase5-v1`、现有 JSON Schema 或 QLoRA 配置；目录创建、文件移动和撤销属于 Rust 应用层，不属于模型学习目标。

## 维护验证

修改阶段五后至少运行：

```powershell
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

如果默认 `target` 中的桌面程序正在运行，可为检查命令指定独立 `--target-dir`，避免 Windows 文件锁冲突。
