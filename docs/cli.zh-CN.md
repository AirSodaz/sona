# Sona CLI

`sona` 现在通过桌面主程序直接提供离线批量转写命令。安装包里的 CLI 子命令由主程序二进制承载，但默认不会帮您写入系统 `PATH`。

当前 CLI 涵盖三个核心工作流：

- 单个本地音频或视频文件的离线转写
- 预置模型列表查看与模型下载
- 启动无头 HTTP API 服务以进行远程集成
- 导出到 `json`、`txt`、`srt`、`vtt` 或 `md`
- 复用与桌面应用相同的预置模型元数据

## 当前范围

- 包含在桌面安装包和应用包中，通过主可执行程序提供
- 支持从源码通过 `cargo run -- ...` 运行
- 尚未包含：Shell `PATH` 注册、实时录音、LLM 润色、LLM 翻译

## 安装位置

CLI 通过打包后的应用程序二进制文件提供，但未在 `PATH` 中注册。

- Windows：在安装目录运行 `Sona.exe transcribe ...`
- macOS：运行 `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux：从安装位置运行 `Sona` 主程序并附带 CLI 子命令
- AppImage：运行挂载后的 AppImage 可执行文件并附带 CLI 子命令

## 常用命令

### 转录文件

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

如果不指定 `--output`，`sona` 会将 JSON 写入 `stdout`。

### 列出模型

```bash
sona models list
```

常用过滤器：

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
```

### 下载模型

```bash
sona models download sherpa-onnx-whisper-turbo
```

如果预置模型需要伴生模型，CLI 会自动下载：

- 需要 VAD 的模型会自动下载 `silero-vad`
- 需要标点的模型会自动下载默认标点模型

您也可以显式覆盖模型目录：

```bash
sona models download silero-vad --models-dir ./models
```

## 配置文件

使用 `--config` 显式传递配置文件。

最小示例：

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
format = "srt"
```

支持的键：

- `models_dir`
- `model_id`
- `vad_model_id`
- `punctuation_model_id`
- `language`
- `threads`
- `enable_itn`
- `vad_buffer_size`
- `format`

命令行标志会覆盖配置文件中的值。

## 必需的伴生模型

某些离线模型需要伴生资产：

- 如果选择的离线模型需要 VAD，必须提供有效的 `vad_model_id`
- 如果选择的离线模型需要标点，必须提供有效的 `punctuation_model_id`

对于 `models download`：

- CLI 会自动下载必需的 VAD 和标点伴生模型

对于 `transcribe`：

- 您仍需传递 `--vad-model-id`、`--punctuation-model-id` 或在配置文件中定义它们
- 缺失必需的伴生 ID 会导致错误并快速失败

## 常用标志

### 全局

```text
sona
  -V, --version
  -v, --verbose
```

使用 `-V` 或 `--version` 打印 Sona 版本号。使用 `-v` 或 `--verbose` 放在子命令前启用详细诊断日志：

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
```

详细诊断日志会写入 `stderr`。命令结果仍写入 `stdout`，包括 `models list` 的 JSON 输出，以及 `transcribe` 未指定 `--output` 时的输出，因此仍可安全管道传给其他工具。

### `transcribe`

```text
sona transcribe <input>
  --config <path>
  --output <path>
  --format <json|txt|srt|vtt|md>
  --language <code>
  --model-id <id>
  --models-dir <path>
  --vad-model-id <id>
  --punctuation-model-id <id>
  --threads <n>
  --enable-itn
  --disable-itn
  --vad-buffer <seconds>
  --save-wav <path>
  --quiet
```

### `serve`

```text
sona serve
  --host <ip>
  --port <port>
  --api-key <key>
  --models-dir <path>
  --ip-whitelist <rules>
  --max-streaming <n>
```

有关 API 使用的更多详细信息，请参阅 [api.zh-CN.md](api.zh-CN.md)。

### `models list`

```text
sona models list
  --models-dir <path>
  --mode <streaming|offline>
  --type <type>
  --language <code>
  --installed
```

注意：

- `--type` 可以是 `whisper`、`vad` 或 `punctuation` 等值
- `--language` 匹配语言令牌，如 `zh`、`en`、`ja` 或 `yue`
- 当前默认输出格式为 JSON

### `models download`

```text
sona models download <model_id>
  --models-dir <path>
  --quiet
```

## 帮助命令

```bash
sona --help
sona help
sona --version
sona -V
sona transcribe --help
sona models --help
sona models list --help
sona models download --help
sona serve --help
```
