# Sona CLI

`sona-cli` is a stateless command-line transcription host. It does not open or manage the Sona SQLite application database, project database, history storage, sync state, or online LLM tasks. Transcripts are written to stdout or to an explicitly supplied output file.

The standalone CLI ships these commands:

- `path-status`
- `init-config`
- `models list|download|delete`
- `diagnostics`
- `export transcript`
- `serve` (local REST transcription with local ASR)
- `transcribe` (local or online batch ASR)
- `transcribe-live` (local or online streaming ASR)

## Run It

```bash
cargo run -p sona-cli -- <command> ...
```

Examples:

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

## Stateless Boundary

The CLI deliberately excludes SQLite, History, Tag, application backup/recovery, Sync, and Online LLM. Do not add commands that silently create or modify the desktop application data directory. Use `export transcript` and stdout/file output to compose the CLI with other tools.

## `path-status`

Resolve one filesystem path through the shared runtime status contract and print JSON to stdout.

```bash
sona-cli path-status ./models
```

## `init-config`

Create a commented TOML starter file for local transcription and the local API server.

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml --force
```
Existing files are protected unless `--force` is supplied. Status text is written to stderr.
When `sona-cli.toml` is present in the current working directory, `transcribe`, `transcribe-live`, and `serve` automatically load it if `-c / --config` is omitted. You can also point to a config file globally via the `SONA_CONFIG` environment variable.

## `models`

List, download, or delete preset local ASR models. These commands operate only on the selected models directory, not on SQLite application state. If `--models-dir` is omitted, Sona checks the `SONA_MODELS_DIR` environment variable before falling back to the desktop app location.

```bash
sona-cli models list
sona-cli models list turbo
sona-cli models list -m batch -t whisper
sona-cli models list -l zh -i -j
sona-cli models download whisper-turbo -q
sona-cli models delete whisper-turbo -y
```

`models list` displays canonical short aliases in the `Alias` column (and in the `aliases` JSON field). You can filter models by keyword (`sona-cli models list <QUERY>`).
`models download` and `models delete` support convenient short aliases (such as `whisper-turbo`, `sensevoice`, `paraformer`, `firered`, `qwen3-asr-0.6b`, `vad`, `punct`) alongside full preset IDs. Close-match suggestions are provided when an unknown model ID is entered.
`models delete` prompts for confirmation `[y/N]` when run in an interactive terminal; pass `-y / --yes` in scripts or non-interactive environments.
## `diagnostics`

Build a diagnostics snapshot from facts supplied by the host. This command does not read the application database.

```bash
sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json
```

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

## `export transcript`

Export a JSON array of transcript segments through the shared Core export service.

```bash
sona-cli export transcript -i ./segments.json -o ./transcript.vtt
sona-cli export transcript -i ./segments.json -o ./transcript.srt -m bilingual
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

The format is inferred from the output extension unless `--format` is supplied. Supported formats are `json`, `txt`, `srt`, `vtt`, and `md`; supported modes are `original`, `translation`, and `bilingual`.
## `transcribe`

Transcribe one local audio file, or a video file when using local ASR. Without `--online-provider`, the command uses an installed local Sherpa preset.

```bash
sona-cli transcribe ./sample.wav -m whisper-turbo
sona-cli transcribe ./sample.wav -o ./out.srt
```
If `sona-cli.toml` is present in the current working directory (or set via `SONA_CONFIG`), it is loaded automatically without passing `-c / --config`. Common flags support short options: `-m / --model-id`, `-l / --language`, `-q / --quiet`, `-o / --output`, `-f / --format`, `-c / --config`. Custom FFmpeg path can be specified via `--ffmpeg-path <PATH>` or `ffmpeg_path` in the config file.
With `--online-provider`, the command uploads the local file to the selected provider and writes the result to stdout or the requested output file. You can supply the API key directly via `--api-key <KEY>` or through an environment variable:

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

## Output and Errors

`transcribe` writes JSON to stdout by default. `transcribe-live` emits live text or NDJSON events and optionally writes a final output file. Validation errors exit 2, model errors exit 3, network/provider errors exit 4, and filesystem/input errors exit 5.

Run `sona-cli <command> --help` for command-specific usage.
