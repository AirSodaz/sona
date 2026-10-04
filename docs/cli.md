# Sona CLI

`sona-cli` is a stateless command-line transcription host. It does not open or manage the Sona SQLite application database, project database, history storage, sync state, or online LLM tasks. Transcripts are written to stdout or to an explicitly supplied output file.

The standalone CLI ships these commands:

- `doctor` (system dependencies, audio devices, models, real hardware acceleration [CUDA, Vulkan, Metal], and full-section configuration health check)
- `devices` (list available audio capture / microphone devices with numeric index and default tag)
- `providers` (list supported online ASR providers and their capabilities)
- `config init|path|check|show` (configuration management: generate template, print path, validate syntax, and inspect content)
- `models list|info|download|delete|verify|path` (preset model lifecycle management, supporting multi-model batch download)
- `transcribe` (local or online batch ASR, supporting single file, multiple files, directories, stdin pipe `-`, single-model auto-inference, and `--mode` text format)
- `transcribe-live` (local or online streaming ASR, supporting `--device 0` index/substring selection and `--stream text|ndjson`)
- `serve` (local REST / WebSocket transcription service with local ASR)
- `export` (export transcript segment JSON to multiple subtitle formats, supporting stdin/stdout UNIX pipes)
- `completion` (shell auto-completion for bash, zsh, fish, powershell, elvish)
- `diagnostics` (host facts snapshot reproduction for desktop integration; for routine health checks, use `doctor`)
- `path-status` (shared runtime path status contract inspection)
## Run It

```bash
cargo run -p sona-cli -- <command> ...
```

Examples:

```bash
sona-cli doctor
sona-cli devices
sona-cli config init
sona-cli models download whisper-turbo sensevoice
sona-cli transcribe ./sample.wav -o ./out.srt
cat ./sample.wav | sona-cli transcribe - -o ./out.srt
sona-cli transcribe ./sample.wav --mode bilingual -f srt
sona-cli transcribe-live --device 0 --stream text
sona-cli export -i ./segments.json -o ./transcript.vtt
sona-cli serve -p 14200

## Stateless Boundary

The CLI deliberately excludes SQLite, History, Tag, application backup/recovery, Sync, and Online LLM. Do not add commands that silently create or modify the desktop application data directory. Use `export transcript` and stdout/file output to compose the CLI with other tools.
## `doctor`

Inspect system environment health including FFmpeg executable and version, audio capture devices, models directory and installed presets count, hardware acceleration detection (checks CUDA, Vulkan, Metal, and CPU modes), and configuration file syntax across all sections (`[transcribe]`, `[transcribe_live]`, and `[serve]`).

```bash
sona-cli doctor
sona-cli doctor --json
sona-cli doctor --models-dir ./models --config ./custom.toml
```

Supports `--json` for machine-readable status reports in CI/CD or automation scripts.

## `devices`

List available audio capture (microphone) devices on the current host, with numeric indexes (e.g. `[0]`) and the system default marked as `[default]`. Both indexes and names can be passed directly to `transcribe-live --device`.

```bash
sona-cli devices
sona-cli devices --json
```

## `config`
Unified configuration management command group supporting template generation, path inspection, validation, and content display:

```bash
sona-cli config init                  # Generate default ./sona-cli.toml
sona-cli config init ./custom.toml -F # Force overwrite custom path
sona-cli config path                  # Print resolved active configuration file path
sona-cli config check                 # Validate [transcribe], [transcribe_live], [serve] sections
sona-cli config show                  # Display active configuration file content
```

Configuration search order:
1. Explicit `-c / --config <PATH>` command line flag;
2. `SONA_CONFIG` environment variable;
3. `./sona-cli.toml` in the current working directory;
4. User-level standard config location (Linux: `~/.config/sona/sona-cli.toml`, macOS: `~/Library/Application Support/sona/sona-cli.toml` [or `$XDG_CONFIG_HOME/sona/sona-cli.toml` if set], Windows: `%APPDATA%\sona\sona-cli.toml`).
## `providers`

List supported online ASR providers with their default environment variable names and supported modes (`batch`, `streaming`).

```bash
sona-cli providers
sona-cli providers --json
```

## `path-status`

Resolve one filesystem path through the shared runtime status contract and print JSON to stdout.

```bash
sona-cli path-status ./models
```

## `models`

List, inspect, download, or delete preset local ASR models. These commands operate only on the selected models directory, not on SQLite application state. If `--models-dir` is omitted, Sona checks the `SONA_MODELS_DIR` environment variable before falling back to the desktop app location.

```bash
sona-cli models list
sona-cli models list turbo
sona-cli models list -m batch -t whisper
sona-cli models list -l zh -i -j
sona-cli models info whisper-turbo
sona-cli models info sensevoice --json
sona-cli models download whisper-turbo -q
sona-cli models download whisper-turbo sensevoice
sona-cli models delete --all -y
sona-cli models verify whisper-turbo
sona-cli models verify --all
sona-cli models path
```

`models list` displays canonical short aliases in the `Alias` column (and in the `aliases` JSON field). You can filter models by keyword (`sona-cli models list <QUERY>`), and filter by `--mode` (`live` or `batch`).
`models info` (alias `models inspect`) inspects full metadata for a preset model including name, type, supported modes, full language coverage (untruncated), required companion models, installation status, and download artifact checksums. Supports `--json`.
`models download`, `models delete`, `models info`, and `models verify` support convenient short aliases (such as `whisper-turbo`, `sensevoice`, `paraformer`, `firered`, `qwen3-asr-0.6b`, `vad`, `punct`) alongside full preset IDs. Close-match suggestions are provided when an unknown model ID is entered.
`models verify` validates file integrity of an installed model without re-downloading. Use `--all` to verify every installed model in the models directory.
`models path` prints the resolved absolute path to the local preset models directory.
`models delete` deletes a specified model or all installed preset models with `--all`. It prompts for confirmation `[y/N]` when run in an interactive terminal; pass `-y / --yes` in scripts or non-interactive environments.

## `diagnostics`

Build a diagnostics snapshot from facts supplied by the host. This command does not read the application database.

```bash
sona-cli diagnostics --app-data-dir ./app_data --input ./facts.json
sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json

Input facts JSON format example (`DiagnosticsCoreInput`):

```json
{
  "config": {
    "streamingModelPath": "/path/to/streaming-model",
    "batchModelPath": "/path/to/batch-model",
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

Export a JSON array of transcript segments through the shared Core export service. Can be invoked directly as `sona-cli export` or via `sona-cli export transcript`.

```bash
sona-cli export -i ./segments.json -o ./transcript.vtt
sona-cli export -i ./segments.json -o ./transcript.srt -m bilingual
sona-cli transcribe ./sample.wav | sona-cli export -f srt > ./transcript.srt
cat ./segments.json | sona-cli export -f vtt
```
Input segments JSON format example (array of `TranscriptSegment`):

```json
[
  {
    "id": "segment-1",
    "text": "Hello world",
    "start": 0.0,
    "end": 2.5,
    "isFinal": true,
    "translation": "Bonjour monde"
  }
]
```

The format is inferred from the output extension unless `--format` is supplied (required when outputting to stdout via `-o -` or omitting `-o`). Supported formats are `json`, `txt`, `srt`, `vtt`, and `md`; supported modes are `original`, `translation`, and `bilingual`. Input and output default to `-` (stdin and stdout), enabling seamless UNIX pipeline composition. In an interactive terminal without piped input, `export` exits immediately with guidance to specify `-i/--input`, avoiding indefinite terminal hangs. Existing output files are protected unless `-F / --force` is supplied.

## `transcribe`

Transcribe one or more audio files, video files (local ASR), entire directories, or media piped through standard input (stdin). Without `--online-provider`, the command uses an installed local Sherpa preset.

```bash
sona-cli transcribe ./sample.wav -m whisper-turbo
sona-cli transcribe ./sample.wav -o ./out.srt             # When only 1 batch model is installed, -m is auto-inferred
cat ./sample.wav | sona-cli transcribe - -o ./out.srt        # Accepts standard input via '-'
sona-cli transcribe ./sample.wav --mode bilingual -f srt # Supports original, translation, bilingual
sona-cli transcribe ./meeting1.wav ./meeting2.wav --output-dir ./transcripts -f srt
sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts --recursive -f srt
sona-cli providers                                       # List supported online ASR providers
```
If `sona-cli.toml` is present in the current working directory (or set via `SONA_CONFIG` / user config directory), it is loaded automatically without passing `-c / --config`.
Single-model auto inference: When `-m / --model-id` is omitted and exactly one batch model is installed locally, the CLI automatically selects it.
Piped stdin support: Specify `-` as the input path to stream media from standard input (`cat sample.wav | sona-cli transcribe - -o out.srt`).
Common flags support short options: `-m / --model-id`, `-l / --language`, `-q / --quiet`, `-o / --output`, `-f / --format`, `-c / --config`. `--jobs` defaults to 1 (batch files are currently transcribed sequentially).
Use `--mode <original|translation|bilingual>` (default `original`) to select the output subtitle mode.
Custom FFmpeg path can be specified via `--ffmpeg-path <PATH>` or `ffmpeg_path` in the config file. `--gpu-acceleration` supports `auto`, `cpu`, `vulkan`, `metal`, and `cuda`.
```bash
export GROQ_API_KEY="..."
sona-cli transcribe ./sample.wav --online-provider groq-whisper --format txt
sona-cli transcribe ./sample.wav --online-provider groq-whisper --api-key gsk_... --format txt

export SONA_VOLCENGINE_ASR_API_KEY="..."
sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --output ./out.srt
```

Supported providers include `volcengine-doubao`, `groq-whisper`, `mistral-voxtral`, `openai-whisper`, `deepgram`, `assemblyai`, and `elevenlabs` (dynamically loaded from `online-asr-providers.json`). The API key is read from a provider-specific environment variable by default:

| Provider | Default environment variable |
| --- | --- |
| `volcengine-doubao` | `SONA_VOLCENGINE_ASR_API_KEY` |
| `groq-whisper` | `GROQ_API_KEY` |
| `mistral-voxtral` | `MISTRAL_API_KEY` |
| `openai-whisper` | `OPENAI_API_KEY` |
| `deepgram` | `DEEPGRAM_API_KEY` |
| `assemblyai` | `ASSEMBLYAI_API_KEY` |
| `elevenlabs` | `ELEVENLABS_API_KEY` |
Use `--api-key-env NAME` to select another variable. `--online-config FILE` accepts a JSON object for non-secret endpoint/model overrides; it must not contain `apiKey` or `api_key`.

Local-only flags such as `--model-id`, `--models-dir`, VAD/punctuation options, thread count, GPU mode, and `--save-wav` are rejected when an online provider is selected. `--force` is required to replace an existing output file.

## `transcribe-live`

Transcribe microphone input or headerless 16 kHz mono signed 16-bit little-endian PCM from stdin.
```bash
sona-cli devices
sona-cli transcribe-live --device 0 -m sensevoice --duration 60 -o ./live.srt
sona-cli transcribe-live --device "realtek" --stream text

ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    -m paraformer \
    --stream ndjson
```

Online streaming currently supports `volcengine-doubao`:

```bash
export SONA_VOLCENGINE_ASR_API_KEY="..."
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --online-provider volcengine-doubao --output-format ndjson
```
`--input microphone` uses the default input device unless `--device` supplies a numeric index (e.g. `--device 0`), exact name, or unique substring (e.g. `--device realtek`). `--stream` (or `--stream-format` / `--output-format`) can be `text` or `ndjson`; `--output` writes a final `json`, `txt`, `srt`, `vtt`, or `md` snapshot. `--format` specifies the output file format and requires `--output`. Ctrl+C, stdin EOF, and `--duration` flush and stop the session before exiting.
The same online credential and non-secret config rules as `transcribe` apply. Local-only model and runtime flags are rejected for online streaming.

## `serve`

Run the shared local HTTP API server. The CLI server remains local-ASR-only; use `transcribe` or `transcribe-live` directly for Online ASR.

```bash
sona-cli serve
sona-cli serve -p 14200 --api-key local-secret
sona-cli serve -c ./custom-config.toml
sona-cli serve --ffmpeg-path /usr/bin/ffmpeg
```

When started, the server prints endpoint hints and authentication requirements to stderr.
Core endpoints exposed:
- `GET  /health`: Health check and server status
- `GET  /info`: Server capabilities, model list, and runtime specs
- `POST /v1/transcriptions`: Native Sona batch transcription API
- `POST /v1/audio/transcriptions`: OpenAI-compatible audio transcription API
- `GET  /v1/transcriptions/jobs`: Transcription task status and queue query
- `WS   /v1/streaming`: Real-time streaming WebSocket audio transcription API
When authentication is enabled with `--api-key <KEY>`, client requests to private endpoints must include the HTTP header: `Authorization: Bearer <KEY>`.
## `completion`

Generate shell auto-completion scripts for `bash`, `zsh`, `fish`, `powershell`, or `elvish`.

```bash
sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli
sona-cli completion zsh > ~/.zfunc/_sona-cli
sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish
sona-cli completion powershell >> $PROFILE
```

## Output and Errors

`transcribe` writes JSON to stdout by default. `transcribe-live` emits live text or NDJSON events and optionally writes a final output file. Validation errors exit 2, model errors exit 3, network/provider errors exit 4, and filesystem/input errors exit 5.

Run `sona-cli <command> --help` for command-specific usage.
