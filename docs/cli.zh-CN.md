# Sona CLI

`sona` 通过桌面主程序提供离线转写命令。安装包不会把 `sona` 写入 shell `PATH`，因此需要直接运行已安装的应用二进制文件并附带 CLI 子命令。从源码构建时，也可以用 Cargo 运行同一组命令。

CLI 范围刻意保持精简：单文件和目录离线转写、预置模型列表/下载/删除、无头 HTTP API 服务启动。它不包含实时录音、LLM 润色或 LLM 翻译。

## 运行方式

- Windows：在安装目录运行 `Sona.exe transcribe ...`
- macOS：运行 `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux 安装包：从安装位置运行 `Sona` 主程序并附带 CLI 子命令
- AppImage：运行挂载后的 AppImage 可执行文件并附带 CLI 子命令
- 源码：`cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml`

## 常用命令

### 转写文件

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

不指定 `--output` 时，转写结果会以 JSON 写入 `stdout`。指定 `--output` 时，格式会从文件扩展名推断，除非同时传入 `--format`。

### 转写目录

```bash
sona transcribe \
  --input-dir ./media \
  --output-dir ./transcripts \
  --format srt \
  --recursive \
  --jobs 1 \
  --config ./sona-cli.toml
```

目录模式会为每个受支持媒体文件在 `--output-dir` 中写出一个转写文件。默认只扫描目录直属文件；加入 `--recursive` 后会包含子目录，并保留相对输出路径。转写正文写入文件，`stdout` 会输出 JSON 成功/失败汇总。

### 列出、下载或删除模型

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
sona models download sherpa-onnx-whisper-turbo
sona models delete sherpa-onnx-whisper-turbo
```

当所选预置模型需要伴生模型时，`models download` 会自动下载所需模型，例如 `silero-vad` 或默认标点模型。
`models delete` 只会删除指定模型，不会自动删除伴生模型。

### 启动 API 服务

```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key
```

HTTP API 端点和请求示例见 [api.zh-CN.md](api.zh-CN.md)。

## 配置文件

通过 `--config` 传入 TOML 文件。命令行参数会覆盖配置文件中的值。

最小 `transcribe` 示例：

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
gpu_acceleration = "auto"
format = "srt"
```

### `transcribe` 配置键

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `models_dir` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | CLI 找不到桌面模型目录时请显式传入。 |
| `model_id` | 必选，除非传入 `--model-id` | 离线预置模型 ID | 无 | 用 `sona models list --mode offline` 查看可用 ID。 |
| `vad_model_id` | 条件必选 | 预置模型 ID | 无 | 所选模型需要 VAD 时必选。 |
| `punctuation_model_id` | 条件必选 | 预置模型 ID | 无 | 所选模型需要标点时必选。 |
| `language` | 可选 | `auto` 或模型语言代码，如 `zh`、`en`、`ja` | `auto` | 覆盖自动语言检测。 |
| `threads` | 可选 | 大于 `0` 的整数 | `4` | 识别线程数。 |
| `enable_itn` | 可选 | `true` 或 `false` | `false` | 启用逆文本归一化。 |
| `vad_buffer_size` | 可选 | 大于 `0` 的数字 | `5.0` | VAD 缓冲秒数。 |
| `gpu_acceleration` | 可选 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 使用 `cpu` 可显式关闭 GPU 加速。 |
| `format` | 可选 | `json`、`txt`、`srt`、`vtt`、`md` | 写入 stdout 或目录模式时为 `json`，否则从 `--output` 推断 | 覆盖输出扩展名推断。 |

### `serve` 配置键

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `host` | 可选 | 监听地址 | `0.0.0.0` | 本机访问可用 `127.0.0.1`。 |
| `port` | 可选 | TCP 端口 `0` 到 `65535` | `14200` | API 服务端口。 |
| `api_key` | 可选 | 字符串 | 空 | 为空时请求不需要 Bearer 认证。 |
| `models_dir` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 用于解析已安装模型。 |
| `ip_whitelist` | 可选 | 逗号分隔规则 | `localhost` | 支持 `localhost`、精确 IP、CIDR、`*`，以及 `192.168.*` 这类 IPv4 通配。 |
| `max_streaming` | 可选 | 非负整数 | `2` | 最大并发流式 WebSocket 连接数。 |
| `max_concurrent` | 可选 | 非负整数 | `2` | 最大并发批量任务数。 |
| `max_queue_size` | 可选 | 非负整数 | `100` | `0` 表示队列基本不限。 |
| `max_upload_size_mb` | 可选 | 非负整数 | `50` | `0` 表示关闭上传大小限制。 |
| `job_ttl_minutes` | 可选 | 非负整数 | `60` | `0` 表示关闭完成/失败任务清理。 |
| `gpu_acceleration` | 可选 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 本地批量和流式任务的服务级默认值。 |
| `vad_model_id` | 可选 | 预置模型 ID | `silero-vad` | API 服务任务的默认 VAD 伴生模型。 |
| `punctuation_model_id` | 可选 | 预置模型 ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API 服务任务的默认标点伴生模型。 |

## 参数

### 全局

```text
sona
  -V, --version
  -v, --verbose
  -h, --help
  help
```

使用 `-V` 或 `--version` 打印 Sona 版本号。使用 `-v` 或 `--verbose` 放在子命令前启用详细诊断日志。使用 `-h`、`--help` 或 `help` 打印命令帮助：

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
sona transcribe --help
```

详细诊断日志会写入 `stderr`。命令结果仍写入 `stdout`，包括 `models list` 的 JSON 输出，以及 `transcribe` 未指定 `--output` 时的输出，因此仍可安全管道传给其他工具。

### `transcribe`

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `<input>` | 必选，除非传入 `--input-dir` | 本地音频或视频文件路径 | 无 | 要转写的单个文件。 |
| `--input-dir <dir>` | 目录模式必选 | 目录路径 | 无 | 转写目录中的受支持媒体文件。 |
| `--config <path>` | 可选 | TOML 文件路径 | 无 | 从配置文件加载默认值。 |
| `--output <path>` | 可选 | 文件系统路径 | `stdout` | 输出文件路径。 |
| `--output-dir <dir>` | 与 `--input-dir` 同用时必选 | 目录路径 | 无 | 为每个输入文件写出一个转写文件。 |
| `--recursive` | 可选 | 标志 | 关闭 | 扫描子目录并保留相对输出路径。 |
| `--jobs <n>` | 可选 | 大于 `0` 的整数 | `1` | 目录模式下最大并发文件任务数。 |
| `--format <format>` | 可选 | `json`、`txt`、`srt`、`vtt`、`md` | 写入 stdout 或目录模式时为 `json`，否则从 `--output` 推断 | 覆盖配置和输出扩展名推断。 |
| `--language <code>` | 可选 | `auto` 或模型语言代码 | `auto` | 覆盖配置。 |
| `--model-id <id>` | 必选，除非配置了 `model_id` | 离线预置模型 ID | 无 | 主转写模型。 |
| `--models-dir <path>` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 覆盖配置。 |
| `--vad-model-id <id>` | 条件必选 | 预置模型 ID | 无 | 所选模型需要 VAD 时必选。 |
| `--punctuation-model-id <id>` | 条件必选 | 预置模型 ID | 无 | 所选模型需要标点时必选。 |
| `--threads <n>` | 可选 | 大于 `0` 的整数 | `4` | 覆盖配置。 |
| `--enable-itn` | 可选 | 标志 | `false` | 与 `--disable-itn` 互斥。 |
| `--disable-itn` | 可选 | 标志 | `false` | 覆盖 `enable_itn = true`；与 `--enable-itn` 互斥。 |
| `--hotwords <words>` | 可选 | 逗号分隔词组 | 无 | 仅 CLI 参数；当前支持 Transducer 和 Qwen3 模型。 |
| `--gpu-acceleration <provider>` | 可选 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | 覆盖配置。 |
| `--vad-buffer <seconds>` | 可选 | 大于 `0` 的数字 | `5.0` | `vad_buffer_size` 的 CLI 参数名。 |
| `--save-wav <path>` | 可选 | 文件系统路径 | 无 | 仅 CLI 参数；保存中间重采样 WAV。与 `--input-dir` 不兼容。 |
| `--quiet` | 可选 | 标志 | 关闭 | 仅 CLI 参数；隐藏转写进度。 |

### `models list`

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `--models-dir <path>` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 用于检测已安装预置模型。 |
| `--mode <mode>` | 可选 | `streaming`、`offline` | 所有模式 | 按支持模式过滤。 |
| `--type <type>` | 可选 | 预置模型类型，如 `whisper`、`vad`、`punctuation` | 所有类型 | 按模型类型过滤。 |
| `--language <code>` | 可选 | 语言令牌，如 `zh`、`en`、`ja`、`yue` | 所有语言 | 按支持语言令牌过滤。 |
| `--installed` | 可选 | 标志 | 关闭 | 只显示 `models_dir` 中已存在的模型。 |
| 输出 | 总是 | JSON | JSON | 写入 `stdout`。 |

### `models download`

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `<model_id>` | 必选 | 已知预置模型 ID | 无 | 要下载的主模型。 |
| `--models-dir <path>` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 目标模型目录。 |
| `--quiet` | 可选 | 标志 | 关闭 | 隐藏单个下载进度。 |
| 伴生模型下载 | 自动 | 所需 VAD 和标点预置模型 | 自动 | 下载主模型时会同时下载必需伴生模型。 |

### `models delete`

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `<model_id>` | 必选 | 已知预置模型 ID | 无 | 要删除的模型。 |
| `--models-dir <path>` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 目标模型目录。 |
| `--yes` | 可选 | 标志 | 关闭 | 跳过交互确认提示。 |
| 安装路径缺失 | 否 | 已知但未安装的预置模型 | 成功 no-op | 向 `stderr` 输出提示并以状态码 0 退出。 |
| 伴生模型删除 | 否 | 所需 VAD 和标点预置模型 | 不删除 | 如果不再需要伴生模型，请显式删除对应模型。 |

### `serve`

| 参数 / 配置键 | 必选性 | 取值范围 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `--config <path>` | 可选 | TOML 文件路径 | 无 | 从配置文件加载默认值。 |
| `--host <ip>` | 可选 | 监听地址 | `0.0.0.0` | 覆盖配置。 |
| `--port <port>` | 可选 | TCP 端口 `0` 到 `65535` | `14200` | 覆盖配置。 |
| `--api-key <key>` | 可选 | 字符串 | 空 | 为空时不启用 Bearer 认证。 |
| `--models-dir <path>` | 可选 | 文件系统路径 | 可推断时使用桌面应用模型目录 | 覆盖配置。 |
| `--ip-whitelist <rules>` | 可选 | 逗号分隔规则 | `localhost` | 支持 `localhost`、精确 IP、CIDR、`*`，以及 `192.168.*` 这类 IPv4 通配。 |
| `--max-streaming <n>` | 可选 | 非负整数 | `2` | 最大并发流式连接数。 |
| `--max-concurrent <n>` | 可选 | 非负整数 | `2` | 最大并发批量任务数。 |
| `--max-queue-size <n>` | 可选 | 非负整数 | `100` | `0` 表示队列基本不限。 |
| `--max-upload-size-mb <n>` | 可选 | 非负整数 | `50` | `0` 表示关闭上传大小限制。 |
| `--job-ttl-minutes <n>` | 可选 | 非负整数 | `60` | `0` 表示关闭完成/失败任务清理。 |
| `--gpu-acceleration <provider>` | 可选 | `auto`、`cpu`、`cuda`、`coreml`、`directml` | `auto` | HTTP API 请求不支持按请求覆盖 GPU 配置。 |
| `--vad-model-id <id>` | 可选 | 预置模型 ID | `silero-vad` | API 服务任务的默认 VAD 伴生模型。 |
| `--punctuation-model-id <id>` | 可选 | 预置模型 ID | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | API 服务任务的默认标点伴生模型。 |

运行 `sona <command> --help` 可查看 clap 生成的完整帮助文本。
