# Sona CLI

`sona` 现在通过桌面主程序直接提供离线批量转写命令。安装包里的 CLI 子命令由主程序二进制承载，源码构建场景也可以通过 Cargo 运行同一套命令。

当前 CLI 已覆盖两类核心工作流：

- 转写单个本地音频或视频文件
- 列出可用预置模型并下载模型
- 以 `json`、`txt`、`srt` 或 `vtt` 导出结果
- 与桌面版共享同一份预置模型元数据

## 当前范围

- 已通过桌面主程序随安装器和应用包一起提供
- 源码运行方式已支持：`cargo run -- ...`
- 暂不包含：系统 `PATH` 注册、实时录音、LLM 润色、LLM 翻译

## 安装包内的位置

CLI 通过已安装的应用主程序提供，但默认不会注册进 `PATH`。

- Windows：在安装目录运行 `Sona.exe transcribe ...`
- macOS：运行 `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux 安装包：从安装位置运行 `Sona` 主程序并附带 CLI 子命令
- AppImage：运行挂载后的 AppImage 可执行文件并附带 CLI 子命令

## 常见命令

### 转写文件

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

如果不传 `--output`，`sona` 会把 `json` 输出到 `stdout`。

### 列出模型

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list
```

常见筛选：

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list --mode offline --type whisper
cargo run --manifest-path src-tauri/Cargo.toml -- models list --language zh --installed
```

### 下载模型

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download sherpa-onnx-whisper-turbo
```

如果模型要求伴随模型，CLI 会自动一起下载：

- 需要 VAD 时会自动下载 `silero-vad`
- 需要标点模型时会自动下载默认标点模型

也可以显式指定模型目录：

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download silero-vad --models-dir ./models
```

## 配置文件

通过 `--config` 显式传入 TOML 配置文件。

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

支持的配置项：

- `models_dir`
- `model_id`
- `vad_model_id`
- `punctuation_model_id`
- `itn_model_ids`
- `language`
- `threads`
- `enable_itn`
- `vad_buffer_size`
- `format`

命令行参数优先级高于配置文件。

## 必需的伴随模型

某些离线模型依赖伴随资产：

- 如果所选离线模型要求 VAD，必须提供可用的 `vad_model_id`
- 如果所选离线模型要求标点，必须提供可用的 `punctuation_model_id`

对于 `models download`：

- CLI 会自动把所需的 VAD / 标点模型一并下载

对于 `transcribe`：

- 仍然需要通过 `--vad-model-id`、`--punctuation-model-id` 或配置文件显式指定 companion model id
- 缺失时会直接失败并给出错误提示

## 常用参数

### `transcribe`

```text
sona transcribe <input>
  --config <path>
  --output <path>
  --format <json|txt|srt|vtt>
  --language <code>
  --model-id <id>
  --models-dir <path>
  --vad-model-id <id>
  --punctuation-model-id <id>
  --itn-model-id <id>    # 可重复
  --threads <n>
  --enable-itn
  --disable-itn
  --vad-buffer <seconds>
  --save-wav <path>
  --quiet
```

### `models list`

```text
sona models list
  --models-dir <path>
  --mode <streaming|offline>
  --type <type>
  --language <code>
  --installed
```

说明：

- `--type` 例如 `whisper`、`vad`、`punctuation`
- `--language` 会按语言 token 匹配，例如 `zh`、`en`、`ja`、`yue`
- 当前默认输出为 JSON

### `models download`

```text
sona models download <model_id>
  --models-dir <path>
  --quiet
```

## help 示例

```bash
sona --help
sona -h
sona --version
sona -V
sona transcribe --help
sona models --help
sona models list --help
sona models download --help
```

## 手工验收

### 验证模型列表

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list --type whisper --language zh
```

预期结果：

- `stdout` 输出 JSON
- 只包含匹配筛选条件的模型

### 验证模型下载

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download sherpa-onnx-whisper-turbo
```

预期结果：

- 下载进度输出到 `stderr`
- 主模型被安装到 models 目录
- 所需的 VAD / 标点模型会自动一起安装

### 验证转写

在本机已经安装桌面版模型的前提下，可以这样验证。只要 `models_dir` 指向桌面版模型目录，这个流程既适用于源码构建，也适用于安装包中的主程序 CLI：

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

预期结果：

- 进度输出到 `stderr`
- 指定的导出文件被创建
- 所选预置模型与伴随模型 id 能在 `models_dir` 下正确解析
