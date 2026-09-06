> 历史资料（2026-09-06）：本文保留旧实现或旧方案原文，不作为当前首版实施指令；状态、范围与验收结论仅适用于当时版本。当前依据见[产品需求](PRD.md)、[路线图](ROADMAP.md)与[历史索引](history/README.md)。

# 本地 Qwen2.5 7B 训练指南：Windows + WSL2 + Ollama

| 属性 | 值 |
| --- | --- |
| 状态 | 训练流程已制定；数据准备、训练和真实验收尚未执行 |
| Windows 用途 | 运行桌面应用与 Ollama |
| WSL2 用途 | 数据准备、QLoRA、权重合并与 GGUF 转换 |
| 基础推理模型 | `qwen2.5:7b` |
| 训练源模型 | `Qwen/Qwen2.5-7B-Instruct` |
| 建议训练方式 | LLaMA-Factory、4-bit QLoRA |
| 建议部署格式 | GGUF `Q4_K_M` |
| 最后核对 | `2026-08-22` |

本指南面向当前 RTX 5060 Laptop 8GB 显存环境。它描述完整、可复现的训练路径，但不表示模型已经训练，也不表示阶段五已经达到发布门槛。

## 1. 先理解整体架构

```text
Windows 桌面应用
  → Rust AiProvider
  → http://127.0.0.1:11434
  → Windows Ollama
  → qwen2.5:7b 或 qwen2.5-file-organizer:7b-q4km-v1

WSL2 Ubuntu
  → 准备训练数据
  → 下载 Hugging Face 原始权重
  → LLaMA-Factory 4-bit QLoRA
  → LoRA adapter
  → 合并为 Hugging Face 模型
  → llama.cpp 转换 GGUF
  → Q4_K_M 量化
  → 复制最终 GGUF 到 Windows
```

各模型文件的职责不能混淆：

| 产物 | 用途 | 能否作为本指南训练源 |
| --- | --- | --- |
| Windows Ollama 中的 `qwen2.5:7b` | 当前应用推理和训练前基线 | 否 |
| Ollama blob/GGUF `Q4_K_M` | 低显存推理 | 否 |
| Hugging Face Safetensors 原始权重 | QLoRA 基础模型 | 是 |
| LoRA adapter | 训练得到的增量权重 | 不能单独部署 |
| merged Hugging Face model | 基础权重与 adapter 合并结果 | 用于转换 |
| GGUF `Q4_K_M` | Windows Ollama 最终部署 | 否 |

Windows 已下载的 Ollama 模型不能直接训练。它已经转换为 GGUF 并经过 `Q4_K_M` 量化，LLaMA-Factory 的 QLoRA 流程需要 Hugging Face Transformers/Safetensors 格式的基础权重。不要尝试从 Ollama blob 逆向恢复训练权重。

## 2. 什么时候才应该训练

训练不是阶段五的第一步。只有同时满足以下条件才进入 QLoRA：

- 程序约束已经能够 100% 拦截危险输出和未知分类；
- 当前提示词已经完成至少一轮优化；
- 已尝试少量高质量示例；
- 已建立 80–120 份非敏感黄金样本；
- 约 40% 黄金样本已锁定为测试集；
- 基础模型仍未达到分类准确率 85% 或文件名可接受率 75%；
- 主要问题确实是分类选择或中文文件命名风格，而不是内容提取、Schema、路径或任务调度错误。

升级顺序固定为：

```text
程序约束修正
  → 提示词优化
  → 少量高质量示例
  → 独立训练集 QLoRA
```

摘要事实性不足时，优先修正正文提取、分段汇总和提示词。不要用少量微调数据“训练事实”。

## 3. 训练前资源清单

### 3.1 需要什么

- Windows 11 与可用的 WSL2；
- WSL2 中的 Ubuntu；
- Windows NVIDIA 驱动，需要支持 CUDA on WSL；
- RTX 5060 Laptop 8GB；
- 建议 32GB 系统内存；16GB 可以试跑，但合并模型可能失败；
- 建议至少 80GB 空闲磁盘；
- 稳定网络，用于首次下载基础模型和依赖；
- Windows Ollama 已运行，且保留 `qwen2.5:7b`；
- 至少 500 条独立训练样本，建议 800–1500 条。

### 3.2 要做什么（Windows PowerShell）

```powershell
wsl --status
wsl --version
wsl --list --verbose
nvidia-smi
Invoke-WebRequest -UseBasicParsing http://127.0.0.1:11434/api/tags
```

如果 WSL 需要更新：

```powershell
wsl --update
wsl --shutdown
```

### 3.3 预期结果

- Ubuntu 的 `VERSION` 为 `2`；
- Windows `nvidia-smi` 能看到 RTX 5060 Laptop；
- Ollama `/api/tags` 返回 HTTP 200；
- 返回模型列表中存在 `qwen2.5:7b`。

### 3.4 失败怎么办

- WSL 不是版本 2：执行 `wsl --set-version <发行版名称> 2`；
- WSL 内核过旧：执行 `wsl --update` 后重启；
- Windows 看不到 GPU：先修复 Windows NVIDIA 驱动；
- Ollama API 不通：启动 Windows Ollama，再检查 11434 端口；
- `ollama` 命令不在 PATH：从 Ollama 安装目录执行，或把安装目录加入当前用户 PATH。

## 4. 安装 WSL2 训练环境

以下命令均在 WSL Bash 中执行，除非标题明确标注 PowerShell。

### 4.1 更新 Ubuntu

需要：WSL2 Ubuntu 能正常联网。

```bash
sudo apt update
sudo apt upgrade -y
sudo apt install -y build-essential git git-lfs curl wget cmake ninja-build pkg-config jq unzip
git lfs install
```

预期：命令无错误结束，`git --version`、`cmake --version` 和 `jq --version` 可用。

### 4.2 验证 WSL GPU

```bash
nvidia-smi
```

预期：WSL 能看到与 Windows 相同的 RTX 5060 Laptop 和约 8GB 显存。

重要：不要在 WSL 安装 Linux NVIDIA 显示驱动，也不要安装会拉取驱动的 `cuda`、`cuda-12-x` 或 `cuda-drivers` 元包。Windows NVIDIA 驱动会把 CUDA 驱动能力映射到 WSL。只有编译 CUDA 程序确实需要时，才按 NVIDIA WSL 文档安装不含驱动的 `cuda-toolkit-12-x`。

### 4.3 安装 Miniconda

```bash
cd /tmp
curl -fLO https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh
bash Miniconda3-latest-Linux-x86_64.sh
```

安装时选择初始化 shell。重新打开 WSL，或者执行安装程序最后提示的初始化命令。然后创建独立环境：

```bash
conda create -n qwen25-train python=3.11 -y
conda activate qwen25-train
python --version
python -m pip install --upgrade pip setuptools wheel
```

预期：`python --version` 显示 Python 3.11.x。不要复用 Windows 中的 Python 3.14。

### 4.4 安装 PyTorch CUDA

RTX 50 系显卡需要包含相应架构支持的较新 PyTorch CUDA 构建。先打开 [PyTorch Get Started](https://pytorch.org/get-started/locally/) 核对当前 Linux/Pip/CUDA 命令。以下是 CUDA 12.8 wheel 的示例：

```bash
python -m pip install torch torchvision --index-url https://download.pytorch.org/whl/cu128
```

随后验证：

```bash
python - <<'PY'
import torch
print("torch:", torch.__version__)
print("cuda runtime:", torch.version.cuda)
print("cuda available:", torch.cuda.is_available())
if torch.cuda.is_available():
    print("gpu:", torch.cuda.get_device_name(0))
    print("capability:", torch.cuda.get_device_capability(0))
    print("bf16:", torch.cuda.is_bf16_supported())
PY
```

必须看到 `cuda available: True` 和 RTX 5060 Laptop。若出现 `no kernel image is available` 或架构不支持警告，停止安装其余依赖，按照 PyTorch 官方选择器升级到支持当前显卡的构建。

### 4.5 安装并固定 LLaMA-Factory

```bash
mkdir -p ~/ai-training/tools ~/ai-training/reports
cd ~/ai-training/tools
git clone https://github.com/hiyouga/LlamaFactory.git
cd LlamaFactory
git rev-parse HEAD | tee ~/ai-training/reports/llamafactory-commit.txt
python -m pip install -e .
python -m pip install -r requirements/metrics.txt
llamafactory-cli version
```

如果 extras 安装未带入 bitsandbytes：

```bash
python -m pip install bitsandbytes
```

保存环境清单：

```bash
mkdir -p ~/ai-training/reports
python -m pip freeze > ~/ai-training/reports/pip-freeze.txt
nvidia-smi > ~/ai-training/reports/nvidia-smi.txt
python - <<'PY' > ~/ai-training/reports/torch-info.txt
import torch
print(torch.__version__)
print(torch.version.cuda)
print(torch.cuda.get_device_name(0) if torch.cuda.is_available() else "CUDA unavailable")
PY
```

不要把依赖写成永远浮动的“最新版”。首次成功试跑后，记录 LLaMA-Factory commit 和 `pip-freeze.txt`，后续实验复用同一环境。

## 5. 规划 WSL 目录

### 5.1 创建目录

```bash
export AI_TRAIN_ROOT="$HOME/ai-training"
mkdir -p \
  "$AI_TRAIN_ROOT/configs" \
  "$AI_TRAIN_ROOT/data/private" \
  "$AI_TRAIN_ROOT/data/processed" \
  "$AI_TRAIN_ROOT/data/manifests" \
  "$AI_TRAIN_ROOT/models" \
  "$AI_TRAIN_ROOT/outputs/adapters" \
  "$AI_TRAIN_ROOT/outputs/merged" \
  "$AI_TRAIN_ROOT/outputs/gguf" \
  "$AI_TRAIN_ROOT/reports" \
  "$AI_TRAIN_ROOT/tools"
chmod 700 "$AI_TRAIN_ROOT/data/private"
```

### 5.2 存储规则

- 模型、训练集和 checkpoint 放在 `/home/<user>/...` 的 WSL ext4 文件系统；
- 不在 `/mnt/c` 中训练；
- 不把原始正文、训练 JSONL、checkpoint 或 Hugging Face 缓存复制进项目仓库；
- 日志不得打印正文；
- 只将最终 GGUF、不含正文的配置、哈希和评测报告复制到 Windows；
- 不删除基础模型，确保随时可以回退。

## 6. 下载 Hugging Face 训练源模型

### 6.1 为什么必须重新下载

Windows Ollama 中的 `qwen2.5:7b` 是推理产物，不是训练源。WSL 需要单独下载一次 `Qwen/Qwen2.5-7B-Instruct`。下载完成后会保存在 WSL 的 Hugging Face 缓存和指定目录中，后续训练不必重复下载。

### 6.2 Hugging Face 下载

```bash
conda activate qwen25-train
python -m pip install --upgrade huggingface_hub
hf download Qwen/Qwen2.5-7B-Instruct \
  --local-dir "$AI_TRAIN_ROOT/models/Qwen2.5-7B-Instruct"
```

网络无法访问 Hugging Face 时，可使用 ModelScope 备用方案：

```bash
python -m pip install modelscope
modelscope download \
  --model Qwen/Qwen2.5-7B-Instruct \
  --local_dir "$AI_TRAIN_ROOT/models/Qwen2.5-7B-Instruct"
```

### 6.3 检查模型

```bash
cd "$AI_TRAIN_ROOT/models/Qwen2.5-7B-Instruct"
ls -lh
test -f config.json
test -f tokenizer.json
test -f tokenizer_config.json
test -f generation_config.json
test -f model.safetensors.index.json
find . -maxdepth 1 -name '*.safetensors' -type f -print
du -sh .
sha256sum config.json tokenizer_config.json > "$AI_TRAIN_ROOT/reports/base-model-metadata.sha256"
```

如果模型目录缺少配置、tokenizer、索引或 Safetensors 分片，不要开始训练。重新执行下载命令；Hugging Face CLI 会复用已完成文件。

## 7. 建立严格隔离的数据集

### 7.1 数据集合

| 集合 | 建议数量 | 用途 | 能否训练 |
| --- | ---: | --- | --- |
| 提示词开发集 | 黄金样本约 60% | 选择提示词和少样本示例 | 否 |
| 锁定测试集 | 黄金样本约 40% | 最终 A/B 验收 | 绝对不能 |
| 独立训练集 | 最低 450 条 | 更新 LoRA 参数 | 是 |
| 独立验证集 | 最低 50 条 | 选择 checkpoint、观察过拟合 | 否 |

训练与验证样本合计最低 500 条，建议 800–1500 条。黄金集不进入训练集。

### 7.2 冻结分类标签体系

开始标注或评测前，先冻结本轮使用的分类标签体系，并在数据 manifest 和实验记录中保存：

- `taxonomy_version`（例如 `taxonomy-v1`）；
- 全局模板 ID 与模板版本；未使用模板时明确记录为 `null`；
- 每个分类稳定且唯一的 `id`、用户可读 `name` 和 `description`。

训练标签必须使用语义化 `category_id`，例如 `game`、`study`。`name` 用于界面展示，`description` 用于补充语义，二者不能代替输出标签。同一类别不得同时使用 `study` 和 `category_2`；当已经存在语义化标签时，`category_1`、`category_2` 等界面生成的占位 ID 不得写入新的训练集、验证集或黄金集。

分类目录不是模型标签的一部分。训练输入只包含分类 ID、名称和描述，不包含根目录、目标目录或任何本地绝对路径。目标目录在标注或分析时可以尚不存在；应用只会在用户接受建议、预览操作并确认执行后，根据合法分类标签安全创建或复用目录。

### 7.3 覆盖要求

- TXT、MD、文本型 PDF、DOCX 以及常见代码/配置文本文件；
- 短、中、长文档；
- 所有启用分类；
- 合理比例的 `category_id: null`；
- 相似分类、边界样本和无法判断样本；
- 中文为主，保留必要英文专名；
- 文件名风格覆盖日期、主题、主体、版本等真实场景；
- 不包含扫描 PDF、损坏文件或应用不支持的格式正文。

### 7.4 防止泄漏

- 为每个源文档计算内容指纹；
- 同一文档的不同副本、切片和轻微修改归为同一组；
- 按“文档组”划分数据，而不是随机划分单条记录；
- 训练、验证、提示词开发、锁定测试之间不能有相同内容指纹；
- 锁定测试集结果不能用于选择学习率、epoch 或 checkpoint；
- 在 manifest 中只记录样本 ID、集合、内容指纹和标注版本，不记录绝对路径。

### 7.5 旧标签迁移

如果旧数据把同一类别写成 `category_2` 和 `study`，先建立一次性别名表，例如 `category_2 -> study`，然后对训练集、验证集、提示词开发集、锁定测试集和黄金答案执行同一映射。迁移时必须：

- 保持样本 ID、文档分组、内容指纹和原有集合归属不变，避免重新划分导致数据泄漏；
- 同步更新样本提供的可选分类 ID 与 assistant 输出中的 `category_id`；
- 记录新的 `taxonomy_version` 和标注版本，但提示词正文未变化时继续使用 `phase5-v1`；
- 迁移完成后重新运行零训练基线；迁移前后的分数差异属于标签空间修正，不能表述为模型能力提升。

私有训练数据只按上述规则由数据所有者迁移，项目仓库不读取或修改这些数据。

## 8. 标注为 LLaMA-Factory 数据

### 8.1 单条样本格式

使用 OpenAI messages 格式。`assistant.content` 是 JSON 字符串，而不是嵌套对象：

```json
{"messages":[{"role":"system","content":"你是本地文件整理助手。输出必须完全符合固定 JSON 结构；摘要和理由使用中文；不得输出路径、创建目录或改变扩展名。"},{"role":"user","content":"文件名：项目会议记录.md\n可选分类：[{\"id\":\"work\",\"name\":\"工作\",\"description\":\"工作资料\"}]\n正文：\n2026 年项目会议讨论了交付时间与责任人。"},{"role":"assistant","content":"{\"summary\":\"项目会议讨论交付时间与责任分工。\",\"keywords\":[\"项目会议\",\"交付时间\",\"责任人\"],\"suggested_filename\":\"2026年项目交付会议记录.md\",\"category_id\":\"work\",\"confidence\":0.93,\"reason\":\"内容属于项目工作会议，文件名补充了年份和主题。\"}"}]}
```

无法分类样本的 `category_id` 使用 `null`，不能使用字符串 `"null"`。

### 8.2 dataset_info.json

在 `$AI_TRAIN_ROOT/data/processed/dataset_info.json` 注册数据集：

```json
{
  "file_organizer_train": {
    "file_name": "file_organizer_train.jsonl",
    "formatting": "sharegpt",
    "columns": { "messages": "messages" },
    "tags": {
      "role_tag": "role",
      "content_tag": "content",
      "user_tag": "user",
      "assistant_tag": "assistant",
      "system_tag": "system"
    }
  },
  "file_organizer_validation": {
    "file_name": "file_organizer_validation.jsonl",
    "formatting": "sharegpt",
    "columns": { "messages": "messages" },
    "tags": {
      "role_tag": "role",
      "content_tag": "content",
      "user_tag": "user",
      "assistant_tag": "assistant",
      "system_tag": "system"
    }
  }
}
```

训练集和验证集必须提前按文档组划分为两个文件。这样可以防止同一文档的变体被随机拆到训练与验证两侧。两个文件都不能包含黄金样本。

## 9. 训练前数据校验

每条数据至少检查：

- JSONL 每行可解析；
- 顶层只有 `messages`；
- 消息顺序为 system、user、assistant；
- assistant 内容可以再次解析成 JSON；
- 输出只有 `summary`、`keywords`、`suggested_filename`、`category_id`、`confidence`、`reason`；
- 字符串必填项非空；
- 关键词非空且不重复；
- 文件名不包含 `/`、`\\` 或绝对路径；
- 建议扩展名与原文件一致；
- 分类 ID 位于该样本给出的分类列表中，或为 `null`；
- 同一样本的分类 ID 大小写不敏感唯一，并与冻结的模板版本和 `taxonomy_version` 一致；
- 已有语义化标签时，不使用 `category_1`、`category_2` 等占位 ID；
- 置信度在 `[0,1]`；
- 不含用户真实绝对路径；
- 不含未脱敏的敏感信息；
- 所有集合的内容指纹不重复。

校验失败的样本必须修正或排除，不能在训练时静默跳过。

## 10. 建立零训练基线

### 10.1 需要什么

- Windows Ollama 中的 `qwen2.5:7b`；
- 已锁定的黄金测试集；
- 当前提示词版本，例如 `phase5-v1`；
- 相同模板 ID、模板版本、`taxonomy_version`、分类配置、温度、输出长度和测试机器状态。

### 10.2 要做什么（Windows PowerShell）

通过应用逐个分析锁定测试样本并完成人工审查，然后运行：

```powershell
pnpm evaluate:ai `
  docs/evaluation/gold.jsonl `
  docs/evaluation/reviewed-predictions-qwen25-base.jsonl
```

保存报告时记录：模型名称、量化版本、提示词版本、模板 ID、模板版本、`taxonomy_version`、Schema 有效性、危险输出拦截、分类结果、文件名可接受性、摘要评分和延迟。

分类标签规范调整不会修改模型结构、基础权重、QLoRA YAML、量化方式、JSON Schema 或训练命令，也不要求立即重新训练。只有统一标签后的零训练基线仍未达到门槛时，才按既定升级顺序评估是否进行 QLoRA。

### 10.3 进入训练的门槛

程序约束和少样本已优化，但分类准确率仍低于 85% 或文件名可接受率仍低于 75%，才进入下一节。否则保留基础模型，不训练。

## 11. 8GB 显存 QLoRA 配置

将以下内容保存为 `$AI_TRAIN_ROOT/configs/qwen25-7b-file-organizer-qlora.yaml`。路径中的 `<user>` 替换为 WSL 用户名，不要保留占位符。

```yaml
### model
model_name_or_path: /home/<user>/ai-training/models/Qwen2.5-7B-Instruct
trust_remote_code: true
quantization_bit: 4
quantization_method: bnb
quantization_type: nf4
double_quantization: true

### method
stage: sft
do_train: true
finetuning_type: lora
lora_rank: 8
lora_alpha: 16
lora_dropout: 0.05
lora_target: q_proj,v_proj

### dataset
dataset_dir: /home/<user>/ai-training/data/processed
dataset: file_organizer_train
eval_dataset: file_organizer_validation
template: qwen
cutoff_len: 1024
overwrite_cache: false
preprocessing_num_workers: 4
dataloader_num_workers: 0

### output
output_dir: /home/<user>/ai-training/outputs/adapters/qwen25-file-organizer-v1
logging_steps: 5
save_strategy: steps
save_steps: 50
eval_strategy: steps
eval_steps: 50
save_total_limit: 2
plot_loss: true
overwrite_output_dir: false
report_to: none

### train
per_device_train_batch_size: 1
per_device_eval_batch_size: 1
gradient_accumulation_steps: 16
gradient_checkpointing: true
learning_rate: 1.0e-4
num_train_epochs: 2.0
lr_scheduler_type: cosine
warmup_ratio: 0.05
weight_decay: 0.0
max_grad_norm: 1.0
bf16: true
fp16: false
seed: 42
ddp_timeout: 180000000
```

如果前面的 PyTorch 检查显示 BF16 不可用，将 `bf16` 改为 `false`、`fp16` 改为 `true`。不要同时启用二者。

## 12. 小规模试跑

### 12.1 准备

复制 20–50 条非黄金样本形成试跑数据，并临时在 YAML 中增加：

```yaml
max_samples: 50
max_steps: 30
```

### 12.2 运行

```bash
conda activate qwen25-train
cd ~/ai-training/tools/LlamaFactory
CUDA_VISIBLE_DEVICES=0 llamafactory-cli train \
  ~/ai-training/configs/qwen25-7b-file-organizer-qlora.yaml \
  2>&1 | tee ~/ai-training/reports/smoke-train.log
```

另开一个 WSL 终端观察：

```bash
watch -n 1 nvidia-smi
```

### 12.3 必须确认

- 使用的是 RTX 5060 Laptop；
- 峰值显存没有持续贴满后崩溃；
- loss 是有限数字，不是 NaN；
- 可以生成 checkpoint；
- checkpoint 能加载；
- 至少 5 个验证样本仍输出完整 JSON；
- 分类 ID 合法，文件扩展名不变。

试跑完成后移除 `max_samples` 和 `max_steps`，不要误用试跑配置执行正式训练。

## 13. 正式训练

```bash
conda activate qwen25-train
cd ~/ai-training/tools/LlamaFactory
CUDA_VISIBLE_DEVICES=0 llamafactory-cli train \
  ~/ai-training/configs/qwen25-7b-file-organizer-qlora.yaml \
  2>&1 | tee ~/ai-training/reports/qwen25-file-organizer-v1-train.log
```

训练规则：

- 一次实验只改变一个主要变量；
- 第一轮使用固定默认参数；
- 根据验证集选择 checkpoint；
- 不查看锁定测试集来选择 checkpoint；
- 最多保留 2 个 checkpoint；
- 每次训练保存 YAML、LLaMA-Factory commit、基础模型来源和数据 manifest；
- 验证 loss 回升、JSON 有效率下降或文件名机械重复时停止训练；
- 不启用 W&B、SwanLab 等远程日志服务，除非另行完成隐私评估。

### 13.1 OOM 固定降级顺序

每次只执行一步并重新试跑：

1. `cutoff_len: 1024` 改为 `768`；
2. 仍失败则改为 `512`；
3. `lora_rank: 8` 改为 `4`，`lora_alpha: 8`；
4. 暂时关闭训练中验证，训练后单独验证；
5. 仍失败则停止本机 7B 训练，改用更大显存环境。

不要通过关闭 Rust 校验、减少安全字段或改用全参数训练解决 OOM。

## 14. 合并 LoRA adapter

合并时必须加载完整基础权重，不能设置 `quantization_bit`。将以下内容保存为 `$AI_TRAIN_ROOT/configs/qwen25-7b-file-organizer-export.yaml`：

```yaml
model_name_or_path: /home/<user>/ai-training/models/Qwen2.5-7B-Instruct
adapter_name_or_path: /home/<user>/ai-training/outputs/adapters/qwen25-file-organizer-v1
template: qwen
finetuning_type: lora
trust_remote_code: true

export_dir: /home/<user>/ai-training/outputs/merged/qwen25-file-organizer-v1
export_size: 5
export_device: cpu
export_legacy_format: false
```

8GB GPU 不适合在显存中合并完整 7B 模型，因此默认 `export_device: cpu`。这会较慢，并需要较多系统内存。

```bash
cd ~/ai-training/tools/LlamaFactory
llamafactory-cli export \
  ~/ai-training/configs/qwen25-7b-file-organizer-export.yaml \
  2>&1 | tee ~/ai-training/reports/qwen25-file-organizer-v1-export.log
```

预期：merged 目录包含配置、tokenizer、Safetensors 分片和索引。若被系统杀死或内存不足，关闭其他程序、增加 WSL swap 后重试；仍失败则在内存更大的机器完成合并。不要把 adapter 与量化基础模型强行合并。

## 15. 安装 llama.cpp 并转换 GGUF

### 15.1 构建 CPU 工具

```bash
cd ~/ai-training/tools
git clone https://github.com/ggml-org/llama.cpp.git
cd llama.cpp
git rev-parse HEAD | tee ~/ai-training/reports/llama-cpp-commit.txt
cmake -B build -DGGML_CUDA=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j 4
python -m pip install -r requirements.txt
```

转换和量化可以使用 CPU。若确实需要 CUDA 编译，再按照 NVIDIA WSL 文档安装不含驱动的 CUDA Toolkit，并使用 `-DGGML_CUDA=ON` 重新构建。

### 15.2 转换高精度 GGUF

```bash
cd ~/ai-training/tools/llama.cpp
python convert_hf_to_gguf.py \
  ~/ai-training/outputs/merged/qwen25-file-organizer-v1 \
  --outfile ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-f16.gguf \
  --outtype f16
```

### 15.3 量化为 Q4_K_M

```bash
./build/bin/llama-quantize \
  ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-f16.gguf \
  ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-Q4_K_M.gguf \
  Q4_K_M
```

可选质量对照：

```bash
./build/bin/llama-quantize \
  ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-f16.gguf \
  ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-Q5_K_M.gguf \
  Q5_K_M
```

禁止从已经量化的 GGUF 再次量化。必须从 F16/BF16 GGUF 生成每一种目标量化。

### 15.4 冒烟测试和哈希

```bash
./build/bin/llama-cli \
  -m ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-Q4_K_M.gguf \
  -cnv \
  -p '请只输出合法 JSON。'

sha256sum ~/ai-training/outputs/gguf/*.gguf \
  > ~/ai-training/reports/gguf.sha256
```

## 16. 将最终模型复制到 Windows

先在 Windows 创建目录：

```powershell
New-Item -ItemType Directory -Force -Path C:\AIModels
```

在 WSL 中只复制最终部署模型：

```bash
cp ~/ai-training/outputs/gguf/qwen25-file-organizer-v1-Q4_K_M.gguf \
  /mnt/c/AIModels/qwen25-file-organizer-v1-Q4_K_M.gguf
sha256sum /mnt/c/AIModels/qwen25-file-organizer-v1-Q4_K_M.gguf
```

回到 Windows PowerShell 校验：

```powershell
Get-FileHash C:\AIModels\qwen25-file-organizer-v1-Q4_K_M.gguf -Algorithm SHA256
```

Windows 和 WSL 显示的哈希必须一致。

## 17. 导入 Windows Ollama

在 `C:\AIModels\Modelfile.qwen25-file-organizer-v1` 写入：

```text
FROM C:/AIModels/qwen25-file-organizer-v1-Q4_K_M.gguf
PARAMETER temperature 0.1
PARAMETER num_ctx 8192
```

当前 Rust 请求会再次传入 `temperature: 0.1` 和 JSON Schema，因此 Modelfile 不需要复制生产 system prompt，也不要改变 chat template。保留 GGUF 自带的 Qwen chat template。

在 Windows PowerShell 执行：

```powershell
ollama create qwen2.5-file-organizer:7b-q4km-v1 `
  -f C:\AIModels\Modelfile.qwen25-file-organizer-v1
ollama show qwen2.5-file-organizer:7b-q4km-v1
ollama run qwen2.5-file-organizer:7b-q4km-v1
```

检查 API：

```powershell
$body = @{
  model = 'qwen2.5-file-organizer:7b-q4km-v1'
  stream = $false
  messages = @(@{ role = 'user'; content = '返回一个简短的 JSON 对象。' })
} | ConvertTo-Json -Depth 5

Invoke-RestMethod `
  -Method Post `
  -Uri http://127.0.0.1:11434/api/chat `
  -ContentType 'application/json' `
  -Body $body
```

不要删除或覆盖 `qwen2.5:7b`。训练模型不能加载时，基础模型是唯一即时回退。

## 18. 在应用中切换模型

1. 启动 Windows Ollama；
2. 启动当前桌面应用；
3. 在 AI 面板模型输入框填写 `qwen2.5-file-organizer:7b-q4km-v1`；
4. 点击刷新，确认显示“模型已就绪”；
5. 配置并启用至少一个分类；
6. 勾选支持的文件；
7. 发起分析并审查完整 JSON 派生结果。

应用仍通过 Rust Ollama Provider 调用 `127.0.0.1:11434`。不需要修改 Provider、数据库或文件操作接口，也不需要在 WSL 中运行第二个 Ollama。

## 19. 基础模型与训练模型 A/B 验收

两个模型必须使用相同：

- 锁定测试集；
- 分类配置；
- 提示词版本；
- 温度和输出长度；
- 应用版本；
- 人工评分规则；
- 机器负载和模型预热方式。

发布门槛：

| 指标 | 门槛 |
| --- | ---: |
| JSON Schema 有效率 | ≥99% |
| 危险输出拦截率 | 100% |
| 未知分类拦截率 | 100% |
| 分类准确率 | ≥85% |
| 文件名人工可接受率 | ≥75% |
| 摘要人工均分 | ≥4/5 |
| 常见短文档 P95 | ≤30秒 |

训练模型还必须满足至少一项有效提升：

- 分类准确率相对基础模型提升至少 3 个百分点；或
- 文件名可接受率相对基础模型提升至少 5 个百分点。

同时，摘要均分下降不得超过 0.2，Schema 和安全指标不得下降。量化模型不达标时，可比较 Q5_K_M；两个量化版本都不达标时，继续使用 `qwen2.5:7b`。

## 20. 回退流程

训练模型出现异常时：

1. 在应用模型输入框切回 `qwen2.5:7b`；
2. 刷新并确认基础模型就绪；
3. 保留失败实验的 YAML、日志、adapter、哈希和报告；
4. 不修改 Rust Schema 或放宽安全校验；
5. 确认扫描、搜索和手动文件操作仍正常；
6. 根据证据判断问题属于数据、过拟合、量化还是 Ollama 导入；
7. 一次只调整一个因素后重新试跑。

## 21. 常见故障

| 症状 | 先检查 | 处理 |
| --- | --- | --- |
| WSL `nvidia-smi` 不可用 | Windows 驱动、WSL版本 | 更新 Windows 驱动与 WSL；不要装 Linux 显示驱动 |
| PyTorch `cuda=False` | wheel 是否为 CUDA 构建 | 按 PyTorch 官方选择器重装支持当前 GPU 的构建 |
| `no kernel image` | PyTorch是否支持RTX 50系架构 | 升级到支持该架构的 CUDA wheel |
| bitsandbytes 加载失败 | CUDA/PyTorch/bitsandbytes版本 | 在同一 Conda 环境重装并保存版本信息 |
| 下载中断 | 网络与缓存 | 重跑 `hf download`，复用已下载分片 |
| WSL 磁盘不足 | `df -h`、checkpoint数量 | 清理失败 checkpoint；不要删除基础模型与锁定报告 |
| 训练启动 OOM | cutoff、rank、其他GPU进程 | 严格按固定降级顺序处理 |
| loss 为 NaN | FP16/BF16、学习率、坏数据 | 降低学习率、检查数据；一次只改一个因素 |
| loss 很快归零 | 重复样本或泄漏 | 检查内容指纹、训练/验证重复和模板 |
| JSON 字段缺失 | 训练标签与模板 | 检查所有 assistant 内容是否为完整 JSON |
| 分类偏向高频类 | 类别分布 | 增加少数类与难例，不盲目增加 epoch |
| 文件名机械重复 | 标注风格单一或过拟合 | 丰富命名样式、减少 epoch、回退 checkpoint |
| 合并被系统杀死 | RAM与swap | 关闭程序、增加swap或转到更大内存环境 |
| GGUF转换失败 | merged目录是否完整 | 检查config、tokenizer、Safetensors与llama.cpp版本 |
| 量化质量下降 | 是否重复量化 | 从F16/BF16 GGUF重新生成Q4/Q5 |
| Ollama模型不存在 | 模型名称或create失败 | 执行`ollama show`并查看create错误 |
| 应用无法连接Ollama | Windows 11434 | 确认运行的是Windows Ollama，检查API tags |
| P95超过30秒 | 模型预热和上下文 | 统一预热后重测；比较Q4_K_M与基础模型 |

## 22. 实验记录模板

每次训练至少记录：

```text
run_id:
日期:
基础模型及commit:
LLaMA-Factory commit:
llama.cpp commit:
PyTorch/CUDA/bitsandbytes版本:
Windows NVIDIA驱动:
训练数据manifest与哈希:
训练YAML哈希:
随机种子:
最佳checkpoint:
adapter哈希:
merged模型路径:
GGUF量化类型与哈希:
基础模型评测报告:
训练模型评测报告:
是否达到部署门槛:
回退模型:
```

## 23. 官方资料

- [NVIDIA CUDA on WSL 指南](https://docs.nvidia.com/cuda/wsl-user-guide/index.html)
- [PyTorch 安装选择器](https://pytorch.org/get-started/locally/)
- [Qwen2.5-7B-Instruct 模型说明](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct)
- [LLaMA-Factory](https://github.com/hiyouga/LlamaFactory)
- [LLaMA-Factory 数据格式](https://github.com/hiyouga/LlamaFactory/blob/main/data/README.md)
- [LLaMA-Factory 示例配置](https://github.com/hiyouga/LlamaFactory/tree/main/examples)
- [llama.cpp](https://github.com/ggml-org/llama.cpp)
- [llama.cpp 量化说明](https://github.com/ggml-org/llama.cpp/blob/master/tools/quantize/README.md)
- [Ollama 导入模型](https://github.com/ollama/ollama/blob/main/docs/import.mdx)
- [Ollama Modelfile](https://github.com/ollama/ollama/blob/main/docs/modelfile.mdx)

执行训练前应重新核对这些官方页面，因为 PyTorch CUDA wheel、LLaMA-Factory 参数和 Ollama 命令可能随版本变化。
