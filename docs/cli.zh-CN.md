# Sona CLI

`sona-cli` 是一个无状态的命令行转写 Host。它不会打开或管理 Sona 的 SQLite 应用数据库、项目数据库、历史存储、同步状态或 Online LLM 任务。转录结果只写入 stdout，或写入命令明确指定的输出文件。

当前独立 CLI 提供以下命令：

- `path-status`
- `init-config`
- `models list|download|delete`
- `diagnostics`
- `export transcript`
- `serve`（使用本地 ASR 的本地 REST 转写）
- `transcribe`（本地或在线批量 ASR）
- `transcribe-live`（本地或在线流式 ASR）

## 运行方式

```bash
cargo run -p sona-cli -- <command> ...
```

示例：

```bash
cargo run -p sona-cli -- path-status ./models
cargo run -p sona-cli -- init-config
cargo run -p sona-cli -- models list -j
cargo run -p sona-cli -- transcribe ./sample.wav -m whisper-turbo
cargo run -p sona-cli -- transcribe ./sample.wav --online-provider groq-whisper
cargo run -p sona-cli -- transcribe-live --online-provider volcengine-doubao
cargo run -p sona-cli -- export transcript -i ./segments.json -o ./transcript.vtt
cargo run -p sona-cli -- serve -p 14200
```

## 无状态边界

CLI 有意排除 SQLite、History、Tag、应用备份/恢复、Sync 和 Online LLM。不要增加会隐式创建或修改桌面应用数据目录的命令。请使用 `export transcript` 以及 stdout/文件输出，把 CLI 与其他工具组合起来。

## `path-status`

通过共享运行时状态契约解析一个文件系统路径，并将 JSON 输出到 stdout。

```bash
sona-cli path-status ./models
```

## `init-config`

生成带注释的本地转写和本地 API server 配置模板。

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml --force
```
已有文件默认受保护，只有传入 `--force` 才会覆盖；状态文本写入 stderr。
当当前工作目录下存在 `sona-cli.toml` 时，`transcribe`、`transcribe-live` 与 `serve` 会在省略 `-c / --config` 时自动加载该配置文件。

## `models`

列出、下载或删除本地 ASR 预置模型。这些命令只操作模型目录，不操作 SQLite 应用状态。

```bash
sona-cli models list -m batch -t whisper
sona-cli models list -l zh -i -j
sona-cli models download whisper-turbo -q
sona-cli models delete whisper-turbo -y
```

`models download` 与 `models delete` 支持便捷的简短模型别名（如 `whisper-turbo`、`sensevoice`、`paraformer`、`firered`、`qwen3-asr-0.6b`、`vad`、`punct`），输入未知模型时会提供相似相近名称推荐提示（"Did you mean ...?"）。

`models delete` 在交互式终端下会提示确认 `[y/N]`；在非交互式 Shell/脚本中传入 `-y / --yes`。
## `diagnostics`

根据 Host 提供的事实构造 diagnostics 快照，不读取应用数据库。

```bash
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

## `export transcript`

通过共享 Core export service 导出 transcript segment JSON 数组。

```bash
sona-cli export transcript -i ./segments.json -o ./transcript.vtt
sona-cli export transcript -i ./segments.json -o ./transcript.srt -m bilingual
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

未提供 `--format` 时从输出扩展名推断。支持 `json`、`txt`、`srt`、`vtt`、`md`；模式支持 `original`、`translation`、`bilingual`。
## `transcribe`

转写一个本地音频文件；使用本地 ASR 时也可输入视频。不提供 `--online-provider` 时使用已安装的本地 Sherpa 预置模型。

```bash
sona-cli transcribe ./sample.wav -m whisper-turbo
sona-cli transcribe ./sample.wav -o ./out.srt
```

如果当前目录存在 `sona-cli.toml`，会自动加载而无需手动传入 `-c / --config`。高频参数支持短选项：`-m / --model-id`、`-l / --language`、`-q / --quiet`、`-o / --output`、`-f / --format`、`-c / --config`。
提供 `--online-provider` 后，CLI 会把本地文件上传到指定服务商，并将结果输出到 stdout 或目标文件：

```bash
set GROQ_API_KEY=...
sona-cli transcribe ./sample.wav --online-provider groq-whisper --format txt

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

`--input microphone` 默认使用 CPAL 输入设备；`--device` 必须与 `--list-input-devices` 返回的完整名称匹配。`--output-format` 支持 `text` 和 `ndjson`；`--output` 可写入最终的 `json`、`txt`、`srt`、`vtt` 或 `md` 快照；`--format` 必须同时提供 `--output`。Ctrl+C、stdin EOF 和 `--duration` 都会先 flush/stop 会话再退出。

在线凭据和非敏感配置规则与 `transcribe` 相同。在线流式使用本地模型参数会被拒绝。

## `serve`

从独立 CLI 启动共享的本地 HTTP API server。CLI server 保持本地 ASR-only；Online ASR 请直接使用 `transcribe` 或 `transcribe-live`。

```bash
sona-cli serve
sona-cli serve -p 14200 --api-key local-secret
sona-cli serve -c ./custom-config.toml
```
## 输出和错误

`transcribe` 默认将 JSON 写入 stdout。`transcribe-live` 输出实时 text 或 NDJSON 事件，并可选写入最终文件。参数校验错误退出 2，模型错误退出 3，网络/provider 错误退出 4，文件系统/输入错误退出 5。

可以通过 `sona-cli <command> --help` 查看命令参数。
