# Sona CLI

`sona-cli` 是一个无状态的命令行转写 Host。它不会打开或管理 Sona 的 SQLite 应用数据库、项目数据库、历史存储、同步状态或 Online LLM 任务。转录结果只写入 stdout，或写入命令明确指定的输出文件。

当前独立 CLI 提供以下命令：

- `doctor`（系统依赖、音频设备、模型环境与配置体检）
- `devices`（列出系统可用音频输入麦克风设备）
- `providers`（列出受支持的在线 ASR 服务商清单及支持模式）
- `path-status`
- `init-config`
- `models list|info|download|delete|verify|path`
- `diagnostics`（或 `diagnostics snapshot`）
- `export`（或 `export transcript`）
- `serve`（使用本地 ASR 的本地 REST 转写）
- `transcribe`（本地或在线批量 ASR，支持单文件、多文件、目录批量与 glob 通配符）
- `transcribe-live`（本地或在线流式 ASR）
- `completion`（Shell 自动补全脚本生成：bash, zsh, fish, powershell, elvish）
## 运行方式

```bash
cargo run -p sona-cli -- <command> ...
```

示例：

```bash
sona-cli doctor
sona-cli devices
sona-cli providers
sona-cli models list -j
sona-cli models info whisper-turbo
sona-cli transcribe ./sample.wav -m whisper-turbo -o ./out.srt
sona-cli transcribe ./sample.wav --online-provider groq-whisper
sona-cli transcribe-live -m sensevoice
sona-cli export -i ./segments.json -o ./transcript.vtt
sona-cli serve -p 14200
```

## 无状态边界

CLI 有意排除 SQLite、History、Tag、应用备份/恢复、Sync 和 Online LLM。不要增加会隐式创建或修改桌面应用数据目录的命令。请使用 `export transcript` 以及 stdout/文件输出，把 CLI 与其他工具组合起来。
## `doctor`

检查本地运行环境健康状态，包括 FFmpeg 可执行文件及版本、系统音频捕获输入设备、模型目录状态与已安装预置模型数量、硬件加速支持情况以及 `sona-cli.toml` 配置文件的合法性。

```bash
sona-cli doctor
sona-cli doctor --json
sona-cli doctor --models-dir ./models --config ./custom.toml
```

支持 `--json` 输出结构化健康检查报告（包含 `all_ok` 布尔值与各组件状态），便于 CI/CD 或自动化部署脚本集成。

## `devices`

快速列出当前系统所有可用的音频输入（麦克风）设备，并自动标记当前默认输入设备 `[default]`。

```bash
sona-cli devices
sona-cli devices --json
```

## `providers`

列出所有支持的在线 ASR 服务商清单，展示其服务商 ID、默认环境变量名以及所支持的模式（`batch`、`streaming`）。

```bash
sona-cli providers
sona-cli providers --json
```

## `path-status`

通过共享运行时状态契约解析一个文件系统路径，并将 JSON 输出到 stdout。

```bash
sona-cli path-status ./models
```

## `init-config`

生成带注释的本地转写和本地 API server 配置模板。

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml -F
```
已有文件默认受保护，只有传入 `-F / --force` 才会覆盖；状态文本写入 stderr。
当当前工作目录下存在 `sona-cli.toml` 时，`transcribe`、`transcribe-live` 与 `serve` 会在省略 `-c / --config` 时自动加载该配置文件。也可通过全局环境变量 `SONA_CONFIG` 指定配置文件路径。

## `models`

列出、查看详情、下载或删除本地 ASR 预置模型。这些命令只操作模型目录，不操作 SQLite 应用状态。省略 `--models-dir` 时，CLI 会优先检查 `SONA_MODELS_DIR` 环境变量，未设置时再回退到桌面端模型路径。

```bash
sona-cli models list
sona-cli models list turbo
sona-cli models list -m batch -t whisper
sona-cli models list -l zh -i -j
sona-cli models info whisper-turbo
sona-cli models info sensevoice --json
sona-cli models download whisper-turbo -q
sona-cli models delete whisper-turbo -y
sona-cli models delete --all -y
sona-cli models verify whisper-turbo
sona-cli models verify --all
sona-cli models path
```

`models list` 在表格输出中展示 `Alias` 简短别名列（并在 `--json` 输出中包含 `aliases` 字段）。支持传入关键字参数过滤（`sona-cli models list <QUERY>`），`--mode` 选项支持 `live` 或 `batch`。
`models info`（别名 `models inspect`）用于查看单个预置模型的完整元数据，包括名称、类型、支持模式、完整支持语言清单（不截断）、必需的 Companion 模型、安装状态及 Artifact 下载哈希清单。支持 `--json`。
`models download`、`models delete`、`models info` 与 `models verify` 支持便捷的简短模型别名（如 `whisper-turbo`、`sensevoice`、`paraformer`、`firered`、`qwen3-asr-0.6b`、`vad`、`punct`），输入未知模型时会提供相似相近名称推荐提示（"Did you mean ...?"）。
`models verify` 用于在无需重新下载的情况下校验已安装模型的文件完整性，支持传入具体模型别名或通过 `--all` 全量校验模型目录下所有已安装预置模型。
`models path` 用于直接输出当前解析生效的本地预置模型根目录绝对路径，便于脚本与目录导航。

`models delete` 支持删除单个指定模型或通过 `--all` 批量删除模型目录下所有已安装预置模型。在交互式终端下会提示确认 `[y/N]`；在非交互式 Shell/脚本中传入 `-y / --yes`。
## `diagnostics`

根据 Host 提供的事实构造 diagnostics 快照，不读取应用数据库。

```bash
sona-cli diagnostics --app-data-dir ./app_data --input ./facts.json
sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json
```

输入事实文件 `facts.json` 格式示例（对应 `DiagnosticsCoreInput`）：

```json
{
  "config": {
    "streamingModelPath": "C:/models/sherpa-onnx-streaming-paraformer",
    "batchModelPath": "C:/models/sherpa-onnx-whisper-turbo",
    "vadModelPath": "",
    "punctuationModelPath": "",
    "microphoneId": "default"
  },
  "permissionState": "granted",
  "microphoneProbe": {
    "options": [],
    "available": true,
    "errorMessage": null
  },
  "systemAudioProbe": {
    "options": [],
    "available": false,
    "errorMessage": null
  },
  "voiceTypingReadiness": {
    "state": "ready",
    "lastErrorMessage": null
  }
}
```

## `export`

通过共享 Core export service 导出 transcript segment JSON 数组。可直接执行 `sona-cli export` 或使用子命令 `sona-cli export transcript`。

```bash
sona-cli export -i ./segments.json -o ./transcript.vtt
sona-cli export -i ./segments.json -o ./transcript.srt -m bilingual
sona-cli transcribe ./sample.wav | sona-cli export -f srt > ./transcript.srt
cat ./segments.json | sona-cli export -f vtt
```
输入分段文件 `segments.json` 格式示例（对应 `TranscriptSegment` 数组）：

```json
[
  {
    "id": "segment-1",
    "text": "Hello world",
    "start": 0.0,
    "end": 2.5,
    "isFinal": true,
    "translation": "你好世界"
  }
]
```

未提供 `--format` 时从输出扩展名推断（通过 `-o -` 输出到 stdout 或省略 `-o` 时为必填）。支持 `json`、`txt`、`srt`、`vtt`、`md`；模式 `--mode` 支持 `original`、`translation`、`bilingual`。输入与输出默认均为 `-`（stdin 与 stdout），完全支持标准 UNIX 管道化组合。在交互式终端下若直接执行且未提供管道输入，命令会立即返回提示指定 `-i/--input`，避免终端无提示阻塞。若目标输出文件已存在，需传入 `-F / --force` 确认覆盖。

## `transcribe`

转写一个或多个本地音视频文件，或转写整个目录。不提供 `--online-provider` 时使用已安装的本地 Sherpa 预置模型。

```bash
sona-cli transcribe ./sample.wav -m whisper-turbo
sona-cli transcribe ./sample.wav -o ./out.srt
sona-cli transcribe ./meeting1.wav ./meeting2.wav --output-dir ./transcripts -f srt
sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts --recursive -f srt
sona-cli transcribe --list-providers
```
如果当前目录存在 `sona-cli.toml`（或设置了 `SONA_CONFIG`），会自动加载而无需手动传入 `-c / --config`。高频参数支持短选项：`-m / --model-id`、`-l / --language`、`-q / --quiet`、`-o / --output`、`-f / --format`、`-c / --config`、`-j / --jobs`。支持通过 `--ffmpeg-path <PATH>` 或配置文件中的 `ffmpeg_path` 指定自定义 FFmpeg 路径。`--gpu-acceleration` 支持 `auto`、`cpu`、`vulkan`、`metal` 与 `cuda`。批处理参数 `--jobs` 默认为 1，当前批处理任务按序执行。
使用 `--list-providers` 可以快速查看所有受支持的在线 ASR 服务商 ID、默认环境变量名及支持的转写模式（batch / streaming）。
```bash
set GROQ_API_KEY=...
sona-cli transcribe ./sample.wav --online-provider groq-whisper --format txt
sona-cli transcribe ./sample.wav --online-provider groq-whisper --api-key gsk_... --format txt

set SONA_VOLCENGINE_ASR_API_KEY=...
sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --output ./out.srt
```
支持的 provider 包括 `volcengine-doubao`、`groq-whisper`、`mistral-voxtral`、`openai-whisper`、`deepgram`、`assemblyai` 与 `elevenlabs`（动态同步自 `online-asr-providers.json`）。默认环境变量如下：

| Provider | 默认环境变量 |
| --- | --- |
| `volcengine-doubao` | `SONA_VOLCENGINE_ASR_API_KEY` |
| `groq-whisper` | `GROQ_API_KEY` |
| `mistral-voxtral` | `MISTRAL_API_KEY` |
| `openai-whisper` | `OPENAI_API_KEY` |
| `deepgram` | `DEEPGRAM_API_KEY` |
| `assemblyai` | `ASSEMBLYAI_API_KEY` |
| `elevenlabs` | `ELEVENLABS_API_KEY` |
使用 `--api-key-env NAME` 指定其他变量。`--online-config FILE` 接受用于覆盖 endpoint/model 等非敏感配置的 JSON 对象；其中不得包含 `apiKey` 或 `api_key`。

选择在线 provider 后，`--model-id`、`--models-dir`、VAD/标点参数、线程数、GPU 模式和 `--save-wav` 等本地参数会被拒绝。覆盖已有输出文件必须使用 `--force`。

## `transcribe-live`

实时转写麦克风，或从 stdin 读取无文件头的 16 kHz、单声道、signed 16-bit little-endian PCM。

```bash
sona-cli transcribe-live --list-input-devices
sona-cli transcribe-live -m sensevoice --duration 60 -o ./live.srt

ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    -m paraformer \
    --output-format ndjson
```

在线流式目前支持 `volcengine-doubao`：

```bash
set SONA_VOLCENGINE_ASR_API_KEY=...
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --online-provider volcengine-doubao --output-format ndjson
```

`--input microphone` 默认使用 CPAL 输入设备；`--device` 必须与 `--list-input-devices` 返回的完整名称匹配（`--list-input-devices` 会将系统默认设备标记为 `[default]`）。`--output-format` 支持 `text` 和 `ndjson`；`--output` 可写入最终的 `json`、`txt`、`srt`、`vtt` 或 `md` 快照；`--format` 用于指定输出文件格式并必须同时提供 `--output`。Ctrl+C、stdin EOF 和 `--duration` 都会先 flush/stop 会话再退出。
在线凭据和非敏感配置规则与 `transcribe` 相同。在线流式使用本地模型参数会被拒绝。

## `serve`

从独立 CLI 启动共享的本地 HTTP API server。CLI server 保持本地 ASR-only；Online ASR 请直接使用 `transcribe` 或 `transcribe-live`。

```bash
sona-cli serve
sona-cli serve -p 14200 --api-key local-secret
sona-cli serve -c ./custom-config.toml
sona-cli serve --ffmpeg-path /usr/bin/ffmpeg
```

启动时，服务会在 stderr 打印可用端点以及鉴权提示。
核心暴露端点包括：
- `GET  /health`：健康检查与服务状态
- `GET  /info`：服务能力、模型列表与规格清单
- `POST /v1/transcriptions`：Sona 原生多媒体文件批量转写接口
- `POST /v1/audio/transcriptions`：OpenAI 兼容的标准音频转录接口

当通过 `--api-key <KEY>` 启用鉴权后，客户端请求私有端点需携带 HTTP 头：`Authorization: Bearer <KEY>`。
## `completion`

为 `bash`、`zsh`、`fish`、`powershell` 或 `elvish` 生成 Shell 自动补全脚本。

```bash
sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli
sona-cli completion zsh > ~/.zfunc/_sona-cli
sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish
sona-cli completion powershell >> $PROFILE
```

## 输出和错误

`transcribe` 默认将 JSON 写入 stdout。`transcribe-live` 输出实时 text 或 NDJSON 事件，并可选写入最终文件。参数校验错误退出 2，模型错误退出 3，网络/provider 错误退出 4，文件系统/输入错误退出 5。

可以通过 `sona-cli <command> --help` 查看命令参数。
