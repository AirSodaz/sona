# Sona CLI

`sona-cli` is a stateless command-line transcription host. It does not open or manage the Sona SQLite application database, project database, history storage, sync state, or online LLM tasks. Transcripts are written to stdout or to an explicitly supplied output file.

The standalone CLI ships these commands:

- `doctor` (system dependencies, audio devices, models, and configuration health check)
- `devices` (list available audio capture / microphone devices)
- `providers` (list supported online ASR providers and their capabilities)
- `path-status`
- `init-config`
- `models list|info|download|delete|verify|path`
- `diagnostics` (or `diagnostics snapshot`)
- `export` (or `export transcript`)
- `serve` (local REST transcription with local ASR)
- `transcribe` (local or online batch ASR, supports single file, multiple files, directories, and glob patterns)
- `transcribe-live` (local or online streaming ASR)
- `completion` (shell auto-completion for bash, zsh, fish, powershell, elvish)
## Run It

```bash
cargo run -p sona-cli -- <command> ...
```

Examples:

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

## Stateless Boundary

The CLI deliberately excludes SQLite, History, Tag, application backup/recovery, Sync, and Online LLM. Do not add commands that silently create or modify the desktop application data directory. Use `export transcript` and stdout/file output to compose the CLI with other tools.
## `doctor`

Inspect system environment health including FFmpeg executable and version, audio capture devices, models directory and installed presets count, hardware acceleration support, and `sona-cli.toml` configuration syntax.

```bash
sona-cli doctor
sona-cli doctor --json
sona-cli doctor --models-dir ./models --config ./custom.toml
```

Supports `--json` for machine-readable status reports in CI/CD or automation scripts.

## `devices`

List available audio capture (microphone) devices on the current host, with the system default marked as `[default]`.

```bash
sona-cli devices
sona-cli devices --json
```

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

## `init-config`

Create a commented TOML starter file for local transcription and the local API server.

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml -F
```
Existing files are protected unless `-F / --force` is supplied. Status text is written to stderr.
When `sona-cli.toml` is present in the current working directory, `transcribe`, `transcribe-live`, and `serve` automatically load it if `-c / --config` is omitted. You can also point to a config file globally via the `SONA_CONFIG` environment variable.

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
sona-cli models delete whisper-turbo -y
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

Transcribe one or more audio files, video files (local ASR), or entire directories. Without `--online-provider`, the command uses an installed local Sherpa preset.

```bash
sona-cli transcribe ./sample.wav -m whisper-turbo
sona-cli transcribe ./sample.wav -o ./out.srt
sona-cli transcribe ./meeting1.wav ./meeting2.wav --output-dir ./transcripts -f srt
sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts --recursive -f srt
sona-cli transcribe --list-providers
```
If `sona-cli.toml` is present in the current working directory (or set via `SONA_CONFIG`), it is loaded automatically without passing `-c / --config`. Common flags support short options: `-m / --model-id`, `-l / --language`, `-q / --quiet`, `-o / --output`, `-f / --format`, `-c / --config`, `-j / --jobs`. Custom FFmpeg path can be specified via `--ffmpeg-path <PATH>` or `ffmpeg_path` in the config file. `--gpu-acceleration` supports `auto`, `cpu`, `vulkan`, `metal`, and `cuda`. Batch concurrency flag `--jobs` defaults to 1 (batch files are currently transcribed sequentially).
Use `--list-providers` to inspect all supported online ASR providers, their default environment variables, and supported modes (batch / streaming).

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
sona-cli transcribe-live --list-input-devices
sona-cli transcribe-live -m sensevoice --duration 60 -o ./live.srt

ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    -m paraformer \
    --output-format ndjson
```

Online streaming currently supports `volcengine-doubao`:

```bash
export SONA_VOLCENGINE_ASR_API_KEY="..."
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --online-provider volcengine-doubao --output-format ndjson
```
`--input microphone` uses the default CPAL input device unless `--device` supplies an exact name (`--list-input-devices` marks the system default device with `[default]`). `--output-format` can be `text` or `ndjson`; `--output` writes a final `json`, `txt`, `srt`, `vtt`, or `md` snapshot. `--format` specifies the output file format and requires `--output`. Ctrl+C, stdin EOF, and `--duration` flush and stop the session before exiting.
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
