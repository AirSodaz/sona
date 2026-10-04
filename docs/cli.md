# Sona CLI

`sona-cli` is a stateless command-line transcription host. It does not open or manage the Sona SQLite application database, project database, history storage, sync state, or online LLM tasks. Transcripts are written to stdout or to an explicitly supplied output file.

The standalone CLI ships these commands:

- `doctor` (system dependencies, audio devices, models, real hardware acceleration [CUDA, Vulkan, Metal], and full-section configuration health check)
- `devices` (list available audio capture / microphone devices with numeric index and default tag)
- `providers` (list supported online ASR providers and recommended models, their capabilities, and current environment configuration status)
- `config init|path|check|show|get|set|edit` (configuration management: generate template, print path, validate syntax, inspect content, read specific keys with `get`, non-interactively set keys with `set`, and safely edit with `$EDITOR` via `edit`; subcommands fully support `--global / --user`)
- `models list|info|download|delete|verify|path` (preset model lifecycle management: list shows standalone ASR models by default and supports `-a/--all` and `-r/--recommended`; verify defaults to all installed models; download supports multiple models and `--mirror`)
- `transcribe` (local or online batch ASR, supporting single file, multiple files, directories, stdin pipe `-` or auto-detected pipe, `--continue-on-error` batch fault tolerance, `--online-model` direct parameter, and `--mode` text format)
- `transcribe-live` (visible alias: `live`, local or online streaming ASR, supporting `--device` index/substring selection, `--stream text|ndjson`, and file export `-f/--format/--export-format` with `--mode`)
- `export` (export transcript segments to srt, vtt, txt, json, or md, supporting positional file arguments or stdin/stdout unix pipes)
- `serve` (run the shared local HTTP and WebSocket API server)
- `completion` (shell auto-completion for bash, zsh, fish, powershell, elvish)
## Run It

```bash
cargo run -p sona-cli -- <command> ...
```

Examples:

```bash
sona-cli doctor
sona-cli devices
sona-cli config init --global
sona-cli models list --recommended
sona-cli models download whisper-turbo --mirror hf-mirror
sona-cli transcribe ./sample.wav -o ./out.srt
cat ./sample.wav | sona-cli transcribe -o ./out.srt
sona-cli transcribe ./sample.wav --provider groq-whisper
sona-cli live --device 0 --stream text -o ./out.srt --mode bilingual
sona-cli export ./segments.json -o ./transcript.vtt
sona-cli serve -p 14200
```

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
sona-cli config init --global         # Write to standard user configuration path (alias: --user / -g)
sona-cli config init ./custom.toml -F # Force overwrite custom path
sona-cli config path                  # Print resolved active configuration file path
sona-cli config check                 # Validate [transcribe], [transcribe_live], [serve] sections
sona-cli config show                  # Display active configuration file content
sona-cli config get transcribe.model_id        # Read a specific configuration value
sona-cli config set transcribe.model_id whisper-turbo # Set configuration value directly (preserves comments & validates)
sona-cli config set serve.port 14200 --global # Write to standard user configuration path (--global / --user)
sona-cli config edit                          # Open config in $EDITOR and validate syntax upon exit

Configuration search order:
1. Explicit `-c / --config <PATH>` command line flag;
2. `SONA_CONFIG` environment variable;
3. `./sona-cli.toml` in the current working directory;
4. User-level standard config location (Linux: `~/.config/sona/sona-cli.toml`, macOS: `~/Library/Application Support/sona/sona-cli.toml` [or `$XDG_CONFIG_HOME/sona/sona-cli.toml` if set], Windows: `%APPDATA%\sona\sona-cli.toml`).

## `providers`

List supported online ASR providers with their default environment variable names, current configuration status (`configured` / `not set`), supported modes (`batch`, `streaming`), and supported models.

```bash
sona-cli providers                        # Overview summary table of providers
sona-cli providers --models               # List all providers with their curated models (defaults tagged [default])
sona-cli providers groq-whisper           # Inspect details, supported models, and usage examples for one provider
sona-cli providers --json                 # Machine-readable JSON output for all providers including models
sona-cli providers groq-whisper --json    # Machine-readable JSON output for a specific provider
```

In `--json` mode, each provider object includes `"configured": true | false` and a full `"models"` array.

## `models`

List, inspect, download, or delete preset local ASR models. These commands operate only on the selected models directory, not on SQLite application state. If `--models-dir` is omitted, Sona checks the `SONA_MODELS_DIR` environment variable before falling back to the desktop app location.

```bash
sona-cli models list
sona-cli models list --recommended    # Show only recommended preset models (short flag: -r)
sona-cli models list --all            # Include auxiliary companion models (VAD, punctuation, diarization; short flag: -a)
sona-cli models list turbo
sona-cli models list --mode batch -t whisper
sona-cli models list -l zh -i -j
sona-cli models info whisper-turbo
sona-cli models info sensevoice --json
sona-cli models download whisper-turbo -q
sona-cli models download whisper-turbo sensevoice
sona-cli models download whisper-turbo --mirror hf-mirror # Options: auto, direct, ghproxy, ghnet, hf-mirror
sona-cli models delete --all -y
sona-cli models verify                # Verifies all installed models by default
sona-cli models verify whisper-turbo  # Verify a specific model
sona-cli models path
```

`models list` displays canonical short aliases in the `Alias` column (and in the `aliases` JSON field). It shows standalone ASR models by default; pass `-a / --all` (or `--all-types`) to view all auxiliary companion models (VAD, punctuation, speaker embedding). You can filter models by keyword (`sona-cli models list <QUERY>`), filter by `--mode` (`live` or `batch`), and filter recommended models with `-r / --recommended`.
`models download` supports `--mirror <auto|direct|ghproxy|ghnet|hf-mirror>` to select download mirror strategy.
`models info` (alias `models inspect`) inspects full metadata for a preset model including name, type, supported modes, full language coverage (untruncated), required companion models, installation status, and download artifact checksums. Supports `--json`.
`models download`, `models delete`, `models info`, and `models verify` support convenient short aliases (such as `whisper-turbo`, `sensevoice`, `paraformer`, `firered`, `qwen3-asr-0.6b`, `vad`, `punct`) alongside full preset IDs. Close-match suggestions are provided when an unknown model ID is entered.
`models verify` validates file integrity of installed models without re-downloading. When `<MODEL_ID>` is omitted, it defaults to verifying all installed models in the models directory. You can also verify a specific model ID or alias.

## `export`

Export a JSON array of transcript segments through the shared Core export service. Can be invoked directly as `sona-cli export` or via `sona-cli export transcript`.

```bash
sona-cli export ./segments.json -o ./transcript.vtt
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
cat ./sample.wav | sona-cli transcribe -o ./out.srt       # Auto-detects piped standard input (or explicitly pass '-')
sona-cli transcribe ./sample.wav --mode bilingual -f srt # Supports original, translation, bilingual
sona-cli transcribe ./meeting1.wav ./meeting2.wav --output-dir ./transcripts -f srt
sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts --recursive -f srt --continue-on-error
sona-cli providers                                       # List supported online ASR providers
```

If `sona-cli.toml` is present in the current working directory (or set via `SONA_CONFIG` / user config directory), it is loaded automatically without passing `-c / --config`.
Single-model auto inference: When `-m / --model-id` is omitted and exactly one batch model is installed locally, the CLI automatically selects it.
Piped stdin support: When standard input is piped, `sona-cli transcribe` automatically streams from standard input even without specifying `-`.
Common flags support short options: `-m / --model-id`, `-l / --language`, `-q / --quiet`, `-o / --output`, `-f / --format`, `-c / --config`, and `--provider` (alias for `--online-provider`). `--jobs` defaults to 1 (batch files are currently transcribed sequentially; concurrent jobs are experimental).
Use `--mode <original|translation|bilingual>` (default `original`) to select the output subtitle mode.
Custom FFmpeg path can be specified via `--ffmpeg-path <PATH>` or `ffmpeg_path` in the config file. `--gpu-acceleration` supports `auto`, `cpu`, `vulkan`, `metal`, and `cuda`.

```bash
export GROQ_API_KEY="..."
sona-cli transcribe ./sample.wav --online-provider groq-whisper --format txt
sona-cli transcribe ./sample.wav --online-provider groq-whisper --online-model whisper-large-v3 --online-param temperature=0.2
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

Use `--api-key-env NAME` to select another variable. Direct CLI overrides `--online-model <NAME>` and repeated `--online-param KEY=VALUE` allow setting model names and non-secret provider options directly without writing JSON files (sensitive credentials like API keys cannot be passed via `--online-param`). `--online-config FILE` accepts a JSON object for overrides as well.

Local-only flags such as `--model-id`, `--models-dir`, VAD/punctuation options, thread count, GPU mode, and `--save-wav` are rejected when an online provider is selected. `--force` is required to replace an existing output file.

## `transcribe-live`

Transcribe microphone input or headerless 16 kHz mono signed 16-bit little-endian PCM from stdin.

```bash
sona-cli devices
sona-cli live -m sensevoice --duration 60 -o ./live.srt
sona-cli transcribe-live --device 0 -m sensevoice --duration 60 -o ./live.srt --mode bilingual
sona-cli transcribe-live --device "realtek" --stream text

ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli live --input stdin \
    -m paraformer \
    --stream ndjson
```

`sona-cli live` is a visible alias for `sona-cli transcribe-live` (displayed in `--help` and shell auto-completion). When exactly one streaming model is installed locally and `-m` is omitted, the CLI automatically selects it.
`--device` accepts a numeric index (e.g. `--device 0`), exact name, or unique substring (e.g. `--device realtek`). `--stream` (alias `--stream-format`) selects live stdout format (`text` or `ndjson`), corresponding to `stream_format` (or legacy `output_format`) in `[transcribe_live]`. When saving to an output file (`-o / --output`), `-f / --format` (alias `--export-format`) specifies the output file format (can also be configured via `format` under `[transcribe_live]`), and `--mode` selects subtitle export mode (`original`, `translation`, or `bilingual`); `--mode` does not alter the live terminal stream.
Supported streaming online providers include `volcengine-doubao`, `mistral-voxtral`, `deepgram`, `assemblyai`, and `elevenlabs`.

```bash
export SONA_VOLCENGINE_ASR_API_KEY="..."
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live --input stdin \
    --online-provider volcengine-doubao --stream ndjson
```

`--input microphone` uses the default input device unless `--device` supplies a numeric index (e.g. `--device 0`), exact name, or unique substring (e.g. `--device realtek`). `--stream` (or `--stream-format`) can be `text` or `ndjson`; `--output` writes a final `json`, `txt`, `srt`, `vtt`, or `md` snapshot. `--format` (alias `--export-format`) specifies the output file format and requires `--output`. Ctrl+C, stdin EOF, and `--duration` flush and stop the session before exiting.
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

# Windows PowerShell (safe append with UTF-8 encoding)
if (!(Test-Path -Path (Split-Path $PROFILE))) { New-Item -ItemType Directory -Path (Split-Path $PROFILE) -Force }
sona-cli completion powershell | Out-File -Append -Encoding utf8 $PROFILE
```

## Internal & Integration Commands (Hidden)

The following commands are hidden from `--help` by default and are intended for host/desktop integration, facts snapshot construction, and automated path contract inspection:

### `path-status`

Resolve one filesystem path through the shared runtime status contract and print JSON to stdout.

```bash
sona-cli path-status ./models
```

### `diagnostics`

Build a diagnostics snapshot from facts supplied by the host. This command does not read the application database (for routine environment checks, use `doctor`).

```bash
sona-cli diagnostics --app-data-dir ./app_data --input ./facts.json
sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json
```

## Output and Errors

`transcribe` writes JSON to stdout by default. `transcribe-live` emits live text or NDJSON events and optionally writes a final output file. Validation errors exit 2, model errors exit 3, network/provider errors exit 4, and filesystem/input errors exit 5.

Run `sona-cli <command> --help` for command-specific usage.
