# Sona CLI

`sona-cli` 是 Sona 的独立命令行程序。桌面端 Tauri 应用已经不再内嵌 CLI 子命令，因此打包后的桌面应用请使用 `sona-cli`，不要再把 `Sona` 当作命令行入口。

当前独立 CLI 已提供的命令如下：

- `path-status`
- `init-config`
- `models list`
- `models download`
- `models delete`
- `history list|query|transcript|snapshots|snapshot|<mutation>`
- `export transcript`
- `serve`
- `transcribe`
- `transcribe-live`

无头 HTTP API 服务由共享的 `sona-api-server` 适配器提供，可从桌面应用或 `sona-cli serve` 启动。

## 运行方式

- 安装包：使用同平台安装包里附带的 `sona-cli`
- 源码：`cargo run -p sona-cli -- <command> ...`

示例：

```bash
cargo run -p sona-cli -- path-status .
cargo run -p sona-cli -- init-config
cargo run -p sona-cli -- models list --json
cargo run -p sona-cli -- history list --app-data-dir ./sona-data --json
cargo run -p sona-cli -- export transcript --input ./segments.json --output ./transcript.vtt
cargo run -p sona-cli -- serve --host 127.0.0.1 --port 14200
cargo run -p sona-cli -- transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
cargo run -p sona-cli -- transcribe-live --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17
```

## 命令

### `path-status`

通过共享的运行时状态契约解析一个文件系统路径，并将 JSON 输出到 `stdout`。

```bash
sona-cli path-status ./models
```

示例输出：

```json
{
  "kind": "directory",
  "path": "C:/work/models",
  "error": null
}
```

### `init-config`

生成一份带注释的起始配置文件，供后续独立 CLI 工作流使用。

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml --force
```

- 默认输出路径：`./sona-cli.toml`
- 已有文件默认受保护，传入 `--force` 才会覆盖
- 状态文本写入 `stderr`

### `models list`

列出预置模型，并可按模式、类型、语言、安装状态过滤。

```bash
sona-cli models list
sona-cli models list --mode offline --type whisper
sona-cli models list --language zh --installed
sona-cli models list --json
```

传入 `--json` 会输出完整的机器可读结构，包括 `install_path`。

### `models download`

下载一个预置模型到解析出的模型目录。

```bash
sona-cli models download sherpa-onnx-whisper-turbo
sona-cli models download silero-vad --models-dir ./models --yes
```

如果目标预置模型依赖伴生资源，`sona-cli` 也会同时下载所需的 VAD 或标点模型。

### `models delete`

删除一个已安装的预置模型。

```bash
sona-cli models delete sherpa-onnx-whisper-turbo --yes
sona-cli models delete silero-vad --models-dir ./models --yes
```

不会自动删除伴生模型。

### `history`

通过共享的 history service 查询或修改已有的 Sona 应用数据目录。

```bash
sona-cli history list --app-data-dir ./sona-data --limit 50 --offset 0
sona-cli history query --app-data-dir ./sona-data --input ./workspace-query.json --json
sona-cli history transcript --app-data-dir ./sona-data --history-id <ID> --json
sona-cli history snapshots --app-data-dir ./sona-data --history-id <ID>
sona-cli history snapshot --app-data-dir ./sona-data --history-id <ID> --snapshot-id <ID> --json
sona-cli history create-live-draft --app-data-dir ./sona-data --id <ID> --audio-extension wav --json
sona-cli history complete-live-draft --app-data-dir ./sona-data --history-id <ID> --segments ./segments.json --duration 12.5 --json
sona-cli history save-recording --app-data-dir ./sona-data --input ./recording.json --audio ./recording.wav --json
sona-cli history import-file --app-data-dir ./sona-data --input ./history-import.json --json
sona-cli history update-transcript --app-data-dir ./sona-data --history-id <ID> --segments ./segments.json --json
sona-cli history create-snapshot --app-data-dir ./sona-data --history-id <ID> --reason polish --segments ./segments.json --json
sona-cli history update-meta --app-data-dir ./sona-data --history-id <ID> --updates ./history-meta.json
sona-cli history assign-project --app-data-dir ./sona-data --history-id <ID> --project-id <PROJECT_ID>
sona-cli history reassign-project --app-data-dir ./sona-data --current-project-id <PROJECT_ID>
sona-cli history delete --app-data-dir ./sona-data --history-id <ID>
```

- `query` 使用与 Tauri、UniFFI 相同的 camelCase `HistoryWorkspaceQueryRequest` JSON 契约。
- `list`、`query`、`snapshots` 默认输出表格；`--json` 保留完整的机器可读响应。
- `transcript`、`snapshot` 默认输出 segment 表格。
- `recording.json` 包含 `segments`、`duration`，以及可选的 `projectId`、`audioExtension`；音频字节只从 `--audio` 读取。
- `history-import.json` 使用 camelCase `HistorySaveImportedFileRequest` 契约。`--segments` 文件是 JSON 数组，`--updates` 文件是 JSON 对象。
- `assign-project` 或 `reassign-project` 不提供目标项目参数时，会把记录移回收件箱。
- 应用数据目录必须已经存在；非法 mutation 输入会在 lazy SQLite adapter 打开数据库前被拒绝。

### `export transcript`

通过共享的 core export service 导出已有的 transcript segment JSON 数组。

```bash
sona-cli export transcript --input ./segments.json --output ./transcript.vtt
sona-cli export transcript --input ./segments.json --output ./transcript.srt --mode bilingual
sona-cli export transcript --input ./segments.json --output ./transcript.txt --format txt --json
```

- 未提供 `--format` 时，从输出文件扩展名推断格式。
- 支持格式：`json`、`txt`、`srt`、`vtt`、`md`。
- 支持模式：`original`（默认）、`translation`、`bilingual`。
- `--json` 输出机器可读的目标路径和写入字节数。

### `serve`

从独立 CLI 启动共享的本地 HTTP API 服务。

```bash
sona-cli serve
sona-cli serve --config ./sona-cli.toml
sona-cli serve --host 127.0.0.1 --port 14200 --api-key local-secret
```

- 持续运行直到按 Ctrl+C
- 复用与桌面应用相同的本地批量转写 API server adapter
- `--config` 会读取 `init-config` 生成的 `[serve]` 配置段
- CLI 侧支持本地 REST 转写。`serve` 的 WebSocket 流式路由和在线 ASR 集成仍需要桌面运行时；独立本地流式转写请使用 `transcribe-live`。

### `transcribe`

使用离线 ASR 适配器转写一个本地音频或视频文件。

```bash
sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
sona-cli transcribe ./sample.wav --config ./sona-cli.toml --output ./out.srt
sona-cli transcribe ./sample.wav --format txt --quiet
```

- 省略 `--output` 时默认输出到 `stdout`
- 支持导出格式：`json`、`txt`、`srt`、`vtt`、`md`
- `--config` 读取由 `init-config` 生成的带注释 `sona-cli.toml` 模板
- `--force` 允许覆盖已有输出文件

### `transcribe-live`

使用已安装的本地流式模型，实时转写麦克风或 stdin 原始音频。

```bash
sona-cli transcribe-live --list-input-devices
sona-cli transcribe-live \
  --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17 \
  --device "Studio Mic" \
  --duration 60 \
  --output ./live.srt
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live \
    --input stdin \
    --model-id sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en \
    --output-format ndjson
```

- 第一版仅支持本地离线 ASR；所选预设必须声明支持 `streaming`，并已安装到解析后的模型目录。
- 麦克风默认使用 CPAL 默认输入设备；`--device` 必须与 `--list-input-devices` 返回的完整名称精确匹配。
- `--input stdin` 接收无文件头的 16 kHz、单声道、signed 16-bit little-endian PCM；末尾不完整 sample 会作为输入错误返回。
- TTY text 模式原位刷新当前转录；stdout 被重定向时，text 模式只在 flush 后写一次最终快照。
- `--output-format ndjson` 每行输出并立即 flush 一个 JSON 对象。事件类型为 `started`、`update`、`stopped` 和仅运行时失败使用的 `error`；转录字段采用 camelCase。
- Ctrl+C、stdin EOF 和 `--duration` 使用相同的正常收尾流程：排空采集音频、flush/stop ASR、可选写入最终文件、输出 `stopped`，并以 0 退出。
- `--output` 可将最终快照写为 `json`、`txt`、`srt`、`vtt` 或 `md`；默认从扩展名推断，也可用 `--format` 覆盖。覆盖已有文件必须显式传入 `--force`。
- `--config` 读取 `sona-cli.toml` 的 `[transcribe_live]`；命令行值覆盖该分区，该分区继续继承顶层 ASR 共享默认值。最终输出路径、导出格式和覆盖行为只允许通过命令行指定。
- 参数校验错误退出码为 2，模型错误为 3，输入/设备错误为 5。NDJSON 运行时错误还会在非零退出前输出一个 `error` 事件。

## 全局参数

```text
sona-cli
  -V, --version
  -h, --help
```

可以通过 `sona-cli <command> --help` 查看对应命令的参数说明。
