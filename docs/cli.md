# Sona CLI

`sona` exposes offline transcription commands through the main desktop executable. Packaged installs do not add `sona` to your shell `PATH`, so run the installed app binary with CLI subcommands. Source builds can run the same commands with Cargo.

The CLI is intentionally narrow: single-file offline transcription, preset model listing/download/deletion, and headless HTTP API server startup. It does not include live recording, LLM polish, or LLM translation.

## Run It

- Windows: run `Sona.exe transcribe ...` from the installation directory
- macOS: run `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux packages: run the packaged `Sona` binary with CLI subcommands from the install location
- AppImage: run the mounted AppImage executable with CLI subcommands
- Source: `cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml`

## Common Commands

### Transcribe a file

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Without `--output`, transcription writes JSON to `stdout`. With `--output`, the format is inferred from the file extension unless `--format` is provided.

### List, download, or delete models

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
sona models download sherpa-onnx-whisper-turbo
sona models delete sherpa-onnx-whisper-turbo
```

`models download` automatically downloads required companion models, such as `silero-vad` or the default punctuation model, when the selected preset needs them.
`models delete` removes only the specified model. It does not delete companion models automatically.

### Start the API server

```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key
```

For HTTP API endpoints and request examples, see [api.md](api.md).

## Config File

Pass a TOML file with `--config`. Command-line flags override config file values.

Minimal `transcribe` example:

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

### `transcribe` config keys

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `models_dir` | Optional | Filesystem path | Desktop app models directory, when inferable | Pass explicitly if the CLI cannot find desktop models. |
| `model_id` | Required unless `--model-id` is passed | Offline preset model id | None | Use `sona models list --mode offline` to find ids. |
| `vad_model_id` | Conditional | Preset model id | None | Required when the selected model requires VAD. |
| `punctuation_model_id` | Conditional | Preset model id | None | Required when the selected model requires punctuation. |
| `language` | Optional | `auto` or a model language code, such as `zh`, `en`, `ja` | `auto` | Overrides automatic language detection. |
| `threads` | Optional | Integer greater than `0` | `4` | Recognizer thread count. |
| `enable_itn` | Optional | `true` or `false` | `false` | Enables inverse text normalization. |
| `vad_buffer_size` | Optional | Number greater than `0` | `5.0` | VAD buffer size in seconds. |
| `gpu_acceleration` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | Use `cpu` to disable GPU acceleration. |
| `format` | Optional | `json`, `txt`, `srt`, `vtt`, `md` | `json` on stdout, otherwise inferred from `--output` | Overrides output extension inference. |

### `serve` config keys

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `host` | Optional | Bind address | `0.0.0.0` | Use `127.0.0.1` for local-only access. |
| `port` | Optional | TCP port `0` to `65535` | `14200` | API server port. |
| `api_key` | Optional | String | Empty | Empty means requests are not protected by Bearer auth. |
| `models_dir` | Optional | Filesystem path | Desktop app models directory, when inferable | Used to resolve installed models. |
| `ip_whitelist` | Optional | Comma-separated rules | `localhost` | Supports `localhost`, exact IPs, CIDR, `*`, and IPv4 wildcards like `192.168.*`. |
| `max_streaming` | Optional | Non-negative integer | `2` | Maximum concurrent streaming WebSocket connections. |
| `max_concurrent` | Optional | Non-negative integer | `2` | Maximum concurrent batch jobs. |
| `max_queue_size` | Optional | Non-negative integer | `100` | `0` means the queue is effectively unlimited. |
| `max_upload_size_mb` | Optional | Non-negative integer | `50` | `0` disables the upload size limit. |
| `job_ttl_minutes` | Optional | Non-negative integer | `60` | `0` disables completed/failed job cleanup. |
| `gpu_acceleration` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | Server-level default for local batch and streaming jobs. |
| `vad_model_id` | Optional | Preset model id | `silero-vad` | Default companion model for API server jobs. |
| `punctuation_model_id` | Optional | Preset model id | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | Default punctuation companion for API server jobs. |

## Parameters

### Global

```text
sona
  -V, --version
  -v, --verbose
  -h, --help
  help
```

Use `-V` or `--version` to print the Sona version. Use `-v` or `--verbose` before a subcommand to enable detailed diagnostic logs. Use `-h`, `--help`, or `help` to print command help:

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
sona transcribe --help
```

Verbose diagnostics are written to `stderr`. Command output, including JSON output from `models list` and `transcribe` without `--output`, remains on `stdout` so it can still be piped to other tools.

### `transcribe`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<input>` | Required | Local audio or video file path | None | File to transcribe. |
| `--config <path>` | Optional | TOML file path | None | Loads defaults from config. |
| `--output <path>` | Optional | Filesystem path | `stdout` | Output file path. |
| `--format <format>` | Optional | `json`, `txt`, `srt`, `vtt`, `md` | `json` on stdout, otherwise inferred from `--output` | Overrides config and output extension inference. |
| `--language <code>` | Optional | `auto` or a model language code | `auto` | Overrides config. |
| `--model-id <id>` | Required unless `model_id` is configured | Offline preset model id | None | Main transcription model. |
| `--models-dir <path>` | Optional | Filesystem path | Desktop app models directory, when inferable | Overrides config. |
| `--vad-model-id <id>` | Conditional | Preset model id | None | Required if the selected model requires VAD. |
| `--punctuation-model-id <id>` | Conditional | Preset model id | None | Required if the selected model requires punctuation. |
| `--threads <n>` | Optional | Integer greater than `0` | `4` | Overrides config. |
| `--enable-itn` | Optional | Flag | `false` | Conflicts with `--disable-itn`. |
| `--disable-itn` | Optional | Flag | `false` | Overrides `enable_itn = true`; conflicts with `--enable-itn`. |
| `--hotwords <words>` | Optional | Comma-separated words | None | CLI-only; currently supported by Transducer and Qwen3 models. |
| `--gpu-acceleration <provider>` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | Overrides config. |
| `--vad-buffer <seconds>` | Optional | Number greater than `0` | `5.0` | CLI name for `vad_buffer_size`. |
| `--save-wav <path>` | Optional | Filesystem path | None | CLI-only; saves the intermediate resampled WAV. |
| `--quiet` | Optional | Flag | Off | CLI-only; hides transcription progress. |

### `models list`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `--models-dir <path>` | Optional | Filesystem path | Desktop app models directory, when inferable | Used to detect installed presets. |
| `--mode <mode>` | Optional | `streaming`, `offline` | All modes | Filters by supported mode. |
| `--type <type>` | Optional | Preset model type, such as `whisper`, `vad`, `punctuation` | All types | Filters by model type. |
| `--language <code>` | Optional | Language token, such as `zh`, `en`, `ja`, `yue` | All languages | Filters by supported language token. |
| `--installed` | Optional | Flag | Off | Shows only models present in `models_dir`. |
| Output | Always | JSON | JSON | Printed to `stdout`. |

### `models download`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<model_id>` | Required | Known preset model id | None | Main model to download. |
| `--models-dir <path>` | Optional | Filesystem path | Desktop app models directory, when inferable | Target models directory. |
| `--quiet` | Optional | Flag | Off | Hides per-download progress. |
| Companion downloads | Automatic | Required VAD and punctuation presets | Automatic | Downloading a main model also downloads required companions. |

### `models delete`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `<model_id>` | Required | Known preset model id | None | Model to delete. |
| `--models-dir <path>` | Optional | Filesystem path | Desktop app models directory, when inferable | Target models directory. |
| `--yes` | Optional | Flag | Off | Skips the interactive confirmation prompt. |
| Missing install path | No | Known but not installed preset | Successful no-op | Prints a notice to `stderr` and exits with status 0. |
| Companion deletion | No | Required VAD and punctuation presets | Not deleted | Delete companion models explicitly if you no longer need them. |

### `serve`

| Parameter / config key | Required | Range | Default | Notes |
| --- | --- | --- | --- | --- |
| `--config <path>` | Optional | TOML file path | None | Loads defaults from config. |
| `--host <ip>` | Optional | Bind address | `0.0.0.0` | Overrides config. |
| `--port <port>` | Optional | TCP port `0` to `65535` | `14200` | Overrides config. |
| `--api-key <key>` | Optional | String | Empty | Empty means no Bearer auth. |
| `--models-dir <path>` | Optional | Filesystem path | Desktop app models directory, when inferable | Overrides config. |
| `--ip-whitelist <rules>` | Optional | Comma-separated rules | `localhost` | Supports `localhost`, exact IPs, CIDR, `*`, and IPv4 wildcards like `192.168.*`. |
| `--max-streaming <n>` | Optional | Non-negative integer | `2` | Maximum concurrent streaming connections. |
| `--max-concurrent <n>` | Optional | Non-negative integer | `2` | Maximum concurrent batch jobs. |
| `--max-queue-size <n>` | Optional | Non-negative integer | `100` | `0` means the queue is effectively unlimited. |
| `--max-upload-size-mb <n>` | Optional | Non-negative integer | `50` | `0` disables the upload size limit. |
| `--job-ttl-minutes <n>` | Optional | Non-negative integer | `60` | `0` disables completed/failed job cleanup. |
| `--gpu-acceleration <provider>` | Optional | `auto`, `cpu`, `cuda`, `coreml`, `directml` | `auto` | HTTP API requests do not accept a per-request GPU override. |
| `--vad-model-id <id>` | Optional | Preset model id | `silero-vad` | Default VAD companion for API server jobs. |
| `--punctuation-model-id <id>` | Optional | Preset model id | `sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8` | Default punctuation companion for API server jobs. |

Run `sona <command> --help` for the full clap-generated help text.
