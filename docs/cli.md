# Sona CLI

`sona-cli` is a stateless command-line transcription host. It does not open or manage the Sona SQLite application database, History/Tag workspace, sync state, or online LLM tasks. Transcripts are written to stdout or to an explicitly supplied output file.

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
cargo run -p sona-cli -- models list --json
cargo run -p sona-cli -- transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
cargo run -p sona-cli -- transcribe ./sample.wav --online-provider groq-whisper
cargo run -p sona-cli -- transcribe-live --online-provider volcengine-doubao
cargo run -p sona-cli -- export transcript --input ./segments.json --output ./transcript.vtt
cargo run -p sona-cli -- serve --host 127.0.0.1 --port 14200
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

## `models`

List, download, or delete preset local ASR models. These commands operate only on the selected models directory, not on SQLite application state.

```bash
sona-cli models list --mode offline --type whisper
sona-cli models list --language zh --installed --json
sona-cli models download sherpa-onnx-whisper-turbo
sona-cli models delete sherpa-onnx-whisper-turbo --yes
```

## `diagnostics`

Build a diagnostics snapshot from facts supplied by the host. This command does not read the application database.

## `export transcript`

Export a JSON array of transcript segments through the shared Core export service.

```bash
sona-cli export transcript --input ./segments.json --output ./transcript.vtt
sona-cli export transcript --input ./segments.json --output ./transcript.srt --mode bilingual
```

The format is inferred from the output extension unless `--format` is supplied. Supported formats are `json`, `txt`, `srt`, `vtt`, and `md`; supported modes are `original`, `translation`, and `bilingual`.

## `transcribe`

Transcribe one local audio file, or a video file when using local ASR. Without `--online-provider`, the command uses an installed local Sherpa preset.

```bash
sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
sona-cli transcribe ./sample.wav --config ./sona-cli.toml --output ./out.srt
```

With `--online-provider`, the command uploads the local file to the selected provider and writes the result to stdout or the requested output file:

```bash
export GROQ_API_KEY="..."
sona-cli transcribe ./sample.wav --online-provider groq-whisper --format txt

export SONA_VOLCENGINE_ASR_API_KEY="..."
sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --output ./out.srt
```

Supported providers are `volcengine-doubao`, `groq-whisper`, and `mistral-voxtral`. The API key is read from a provider-specific environment variable by default:

| Provider | Default environment variable |
| --- | --- |
| `volcengine-doubao` | `SONA_VOLCENGINE_ASR_API_KEY` |
| `groq-whisper` | `GROQ_API_KEY` |
| `mistral-voxtral` | `MISTRAL_API_KEY` |

Use `--api-key-env NAME` to select another variable. `--online-config FILE` accepts a JSON object for non-secret endpoint/model overrides; it must not contain `apiKey` or `api_key`.

Local-only flags such as `--model-id`, `--models-dir`, VAD/punctuation options, thread count, GPU mode, and `--save-wav` are rejected when an online provider is selected. `--force` is required to replace an existing output file.

## `transcribe-live`

Transcribe microphone input or headerless 16 kHz mono signed 16-bit little-endian PCM from stdin.

```bash
sona-cli transcribe-live --list-input-devices
sona-cli transcribe-live \
  --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17 \
  --device "Studio Mic" --duration 60 --output ./live.srt

ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --model-id sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en \
    --output-format ndjson
```

Online streaming currently supports `volcengine-doubao`:

```bash
export SONA_VOLCENGINE_ASR_API_KEY="..."
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --online-provider volcengine-doubao --output-format ndjson
```

`--input microphone` uses the default CPAL input device unless `--device` supplies an exact name. `--output-format` can be `text` or `ndjson`; `--output` writes a final `json`, `txt`, `srt`, `vtt`, or `md` snapshot. `--format` requires `--output`. Ctrl+C, stdin EOF, and `--duration` flush and stop the session before exiting.

The same online credential and non-secret config rules as `transcribe` apply. Local-only model and runtime flags are rejected for online streaming.

## `serve`

Run the shared local HTTP API server. The CLI server remains local-ASR-only; use `transcribe` or `transcribe-live` directly for Online ASR.

```bash
sona-cli serve
sona-cli serve --config ./sona-cli.toml
sona-cli serve --host 127.0.0.1 --port 14200 --api-key local-secret
```

## Output and Errors

`transcribe` writes JSON to stdout by default. `transcribe-live` emits live text or NDJSON events and optionally writes a final output file. Validation errors exit 2, model errors exit 3, network/provider errors exit 4, and filesystem/input errors exit 5.

Run `sona-cli <command> --help` for command-specific usage.
