# Sona CLI Guide & Reference

<!-- Generated automatically from `sona_cli::cli_command()`. DO NOT EDIT MANUALLY. -->
<!-- To regenerate or verify: `cargo run -p sona-cli --example generate_cli_docs [-- --check]` (or `pnpm run generate:cli-docs`) -->

`sona-cli` is a standalone, stateless command-line speech-to-text host backed by `sona-core`. It transcribes audio files, directories, live microphone input, and standard input streams with local preset ASR models or online cloud providers.

## Stateless Boundary

The CLI deliberately excludes SQLite, History, Tag, application backup/recovery, Sync, and Online LLM tasks. It will not silently create or mutate desktop application data directories. Output is emitted to stdout or written to explicitly specified target files.

## Configuration Precedence

Configuration options are resolved in the following priority order:

1. Explicit CLI argument: `-c / --config <PATH>`;
2. Global environment variable: `SONA_CONFIG` (if set and non-empty);
3. Local configuration file in current working directory: `./sona-cli.toml` (if the file exists);
4. User standard configuration file: `sona-cli.toml` (if the file exists; Linux: `~/.config/sona/sona-cli.toml`, macOS: `~/Library/Application Support/sona/sona-cli.toml`, Windows: `%APPDATA%\sona\sona-cli.toml` or `%USERPROFILE%\.config\sona\sona-cli.toml`);
5. Built-in defaults: if none of the above files exist, the CLI runs with built-in defaults without error.

## Exit Codes & Errors

- `0`: Success;
- `1`: General failure (e.g. `doctor --strict` health check failure, serialization error);
- `2`: Validation or CLI usage error (invalid argument, missing provider, nonexistent input file, duration <= 0);
- `3`: Model error (missing preset, corrupted download, uninstalled companion);
- `4`: Network or online provider error (authentication failure, API timeout);
- `5`: Filesystem error (output file already exists without `--force`, directory write failure);
- `130`: Cancelled (interrupted by Ctrl-C signal).

## Public Command Reference

### `sona-cli`

Standalone CLI backed by sona-core

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--verbose` | `-v` | - | Enable verbose logging (-v for info, -vv for debug) |

**Usage & Examples:**

```text
Quick Start:
  1. Inspect & download a local ASR model:
       sona-cli models list
       sona-cli models download whisper-turbo
  2. Transcribe an audio or video file:
       sona-cli transcribe ./sample.wav -m whisper-turbo
       sona-cli transcribe ./sample.wav -m whisper-turbo -o ./transcript.srt
  3. Transcribe via cloud provider:
       export GROQ_API_KEY="..."
       sona-cli transcribe ./sample.wav --online-provider groq-whisper
  4. Live streaming transcription:
       sona-cli live -m sensevoice
       ffmpeg -i audio.wav -f s16le -ac 1 -ar 16000 - | \
         sona-cli live --input stdin -m sensevoice
  5. Generate shell completion:
       sona-cli completion bash > /etc/bash_completion.d/sona-cli

Use 'sona-cli <COMMAND> --help' for command-specific options.
```

#### `sona-cli completion`

Generates shell auto-completion scripts

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<SHELL>` | - | - | Target shell to generate completions for: bash, elvish, fish, powershell, zsh |

**Usage & Examples:**

```text
Examples:
  sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli
  sona-cli completion zsh > ~/.zfunc/_sona-cli
  sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish
  sona-cli completion powershell | Out-File -Append -Encoding utf8 $PROFILE
```

#### `sona-cli config`

Inspects and manages Sona CLI configuration

**Usage & Examples:**

```text
Examples:
  sona-cli config init
  sona-cli config init ./custom.toml -F
  sona-cli config path
  sona-cli config check
  sona-cli config show
  sona-cli config get transcribe.model_id
  sona-cli config set transcribe.model_id whisper-turbo
  sona-cli config set serve.port 14200 --global
  sona-cli config edit
```

##### `sona-cli config check`

Validate the syntax and sections of the configuration file

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--config` | `-c` | - | Optional configuration file path override |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config check
  sona-cli config check -c ./custom.toml
```

##### `sona-cli config edit`

Open the configuration file in $EDITOR and validate syntax upon exit

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--config` | `-c` | - | Optional config file to edit |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config edit
  sona-cli config edit -c ./custom.toml
```

##### `sona-cli config get`

Read a specific configuration key (e.g. transcribe.model_id)

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<KEY>` | - | - | Dot-separated key path, e.g. transcribe.model_id or serve.port |
| `--config` | `-c` | - | Optional config file to inspect |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config get transcribe.model_id
  sona-cli config get serve.port
  sona-cli config get transcribe.model_id --global
```

##### `sona-cli config init`

Create a commented TOML starter configuration template

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<PATH>` | - | - | Path to write the commented starter template, default sona-cli.toml |
| `--global` | `-g` | - | Write to user standard configuration path instead of ./sona-cli.toml |
| `--force` | `-F` | - | Overwrite an existing starter template or config file |

**Usage & Examples:**

```text
Examples:
  sona-cli config init
  sona-cli config init ./sona-cli.toml -F
```

##### `sona-cli config path`

Print the resolved active configuration file path

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--config` | `-c` | - | Optional configuration file path override |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config path
  sona-cli config path -c ./custom.toml
```

##### `sona-cli config set`

Set a specific configuration key (e.g. transcribe.model_id whisper-turbo)

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<KEY>` | - | - | Dot-separated key path, e.g. transcribe.model_id or serve.port |
| `<VALUE>` | - | - | Value to set (automatically parses numbers, booleans, and strings) |
| `--config` | `-c` | - | Optional config file to modify |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config set transcribe.model_id whisper-turbo
  sona-cli config set serve.port 14200
  sona-cli config set transcribe.enable_itn true
  sona-cli config set transcribe.model_id whisper-turbo --global
```

##### `sona-cli config show`

Display the contents of the resolved configuration file

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--config` | `-c` | - | Optional configuration file path override |
| `--global` | `-g` | - | Target user standard configuration path instead of local directory |

**Usage & Examples:**

```text
Examples:
  sona-cli config show
  sona-cli config show -c ./custom.toml
```

#### `sona-cli devices`

Lists available audio input (microphone) devices

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--json` | `-j` | - | Print machine-readable JSON |

**Usage & Examples:**

```text
Examples:
  sona-cli devices
  sona-cli devices --json
```

#### `sona-cli doctor`

Checks system dependencies, audio devices, and models directory

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--models-dir` | - | - | Override the target models directory |
| `--config` | `-c` | - | Optional config file to inspect |
| `--ffmpeg-path` | - | - | Custom path to the ffmpeg executable |
| `--json` | `-j` | - | Print machine-readable JSON |
| `--strict` | - | - | Return non-zero exit code if health check fails or has warnings |

**Usage & Examples:**

```text
Examples:
  sona-cli doctor
  sona-cli doctor --json
  sona-cli doctor --models-dir ./models
```

#### `sona-cli export`

Exports transcript JSON segments into subtitle or text files (srt, vtt, txt, md, json)

**Aliases:** `convert`

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<INPUT>` | - | - | Positional input JSON file containing an array of transcript segments |
| `--input` | `-i` | - | JSON file containing transcript segments (alternative to positional INPUT) |
| `--output` | `-o` | `-` | Destination file path, or "-" for stdout |
| `--format` | `-f` | - | Export format: json, txt, srt, vtt, or md; required when output is stdout ("-"), inferred from output file extension otherwise |
| `--mode` | `-m` | `original` | Text selection mode: original, translation, or bilingual |
| `--json` | `-j` | - | Prints JSON summary instead of the default table output (file output only) |
| `--force` | `-F` | - | Overwrite existing destination file |

**Usage & Examples:**

```text
Input JSON format:
  [
    {
      "id": "segment-1",
      "text": "Hello",
      "start": 0.0,
      "end": 2.5,
      "isFinal": true,
      "translation": "Bonjour"
    }
  ]

Supported export formats:
  json, txt, srt, vtt, md (inferred from output file extension when omitted; required when exporting to stdout)

Examples:
  sona-cli export ./segments.json -o ./transcript.srt
  sona-cli convert ./segments.json -o ./transcript.vtt
  cat ./segments.json | sona-cli export -f srt > ./transcript.srt
  sona-cli export ./segments.json -f txt
```

#### `sona-cli live`

Transcribe live audio using local or online ASR

**Aliases:** `transcribe-live`

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--input` | - | - | Live input source |
| `--device` | - | - | Microphone device index (e.g. 0), exact name, or unique substring (e.g. "realtek") |
| `--duration` | - | - | Stop after this many seconds (supports fractional seconds, e.g. 10.5) |
| `--stream` | - | - | Live stdout stream format: text or ndjson (alias: --stream-format) |
| `--output` | `-o` | - | Optional final transcript file |
| `--save-audio` | - | - | Save recorded audio input to a WAV file |
| `--format` | `-f` | - | Final transcript export format (json, txt, srt, vtt, md). Requires --output |
| `--mode` | - | `original` | Text selection mode for final transcript: original, translation, or bilingual |
| `--force` | `-F` | `false` | Overwrite an existing final transcript file |
| `--model` | `-m` | - | Streaming preset model id to use, or online model name when an online provider is selected |
| `--models-dir` | - | - | Models directory containing installed presets |
| `--language` | `-l` | - | Override the language setting |
| `--vad-model-id` | - | - | VAD model id override |
| `--punctuation-model-id` | - | - | Punctuation model id override |
| `--online-provider` | - | - | Use an online ASR provider instead of local Sherpa ASR (alias: --provider) |
| `--api-key` | - | - | Direct API key for the online ASR provider. Takes precedence over --api-key-env |
| `--api-key-env` | - | - | Environment variable containing the online ASR API key. Default env vars: volcengine-doubao: SONA_VOLCENGINE_ASR_API_KEY, groq-whisper: GROQ_API_KEY, mistral-voxtral: MISTRAL_API_KEY, openai-whisper: OPENAI_API_KEY, deepgram: DEEPGRAM_API_KEY, assemblyai: ASSEMBLYAI_API_KEY, elevenlabs: ELEVENLABS_API_KEY |
| `--online-model` | - | - | Override the model name for the online ASR provider (e.g. whisper-large-v3, nova-2) |
| `--online-param` | - | - | Pass arbitrary KEY=VALUE configuration options to the online provider (can be repeated) |
| `--online-config` | - | - | JSON object overriding non-secret provider endpoint or model settings |
| `--threads` | - | - | Number of recognition threads (default: system auto-configured) |
| `--enable-itn` | - | `false` | Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120") |
| `--hotwords` | - | - | Optional hotwords string to enhance recognition, separated by newlines or commas |
| `--gpu-acceleration` | - | - | GPU acceleration mode: auto, cpu, vulkan, metal, or cuda |
| `--vad-buffer` | - | - | VAD buffer size in seconds, for example 0.5 |
| `--sentence-only` | - | `false` | Only transcribe complete sentences upon VAD truncation (reduces compute/battery load) |
| `--config` | `-c` | - | Optional config file, usually sona-cli.toml |

**Usage & Examples:**

```text
Examples:
  sona-cli live -m sensevoice
  sona-cli live -m sensevoice --duration 30 -o ./meeting.srt
  sona-cli live --online-provider volcengine-doubao
  ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
    sona-cli live --input stdin -m sensevoice --stream ndjson
```

#### `sona-cli models`

Manage, download, verify, and inspect preset ASR models

**Usage & Examples:**

```text
Examples:
  sona-cli models list
  sona-cli models list --recommended
  sona-cli models download whisper-turbo
  sona-cli models info whisper-turbo
  sona-cli models verify whisper-turbo
  sona-cli models verify --all
  sona-cli models delete whisper-turbo -y
  sona-cli models path
```

##### `sona-cli models delete`

Deletes an installed preset model from the models directory

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<MODEL_ID>` | - | - | Preset model id(s) or alias(es) to delete, for example sherpa-onnx-whisper-turbo or silero-vad |
| `--models-dir` | - | - | Override the models directory |
| `--yes` | `-y` | - | Delete without prompting for confirmation |
| `--all` | - | - | Delete all installed preset models in the models directory |

**Usage & Examples:**

```text
Examples:
  sona-cli models delete sherpa-onnx-whisper-turbo --models-dir ./models --yes
  sona-cli models delete silero-vad --models-dir ./models --yes
```

##### `sona-cli models download`

Downloads a preset model into the models directory

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<MODEL_IDS>` | - | - | Preset model id(s) or alias(es), for example whisper-turbo or sensevoice |
| `--models-dir` | - | - | Override the target models directory |
| `--quiet` | `-q` | - | Hide per-download progress output |
| `--mirror` | - | - | Download mirror strategy: auto, direct, ghproxy, ghnet, or hf-mirror |
| `--yes` | `-y` | - | Overwrite invalid files without prompting for confirmation |

**Usage & Examples:**

```text
Examples:
  sona-cli models download sherpa-onnx-whisper-turbo
  sona-cli models download silero-vad --models-dir ./models
```

##### `sona-cli models info`

Displays detailed metadata and configuration for a preset model

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<MODEL_ID>` | - | - | Preset model id or alias, for example whisper-turbo or sensevoice |
| `--models-dir` | - | - | Override the target models directory |
| `--json` | `-j` | - | Print machine-readable JSON |

**Usage & Examples:**

```text
Examples:
  sona-cli models info whisper-turbo
  sona-cli models info sensevoice -j
  sona-cli models info silero-vad --models-dir ./models
```

##### `sona-cli models list`

Lists preset models known to the CLI

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--models-dir` | - | - | Override the models directory used to detect installed models |
| `--mode` | - | - | Filter by mode: live or batch |
| `--type` | `-t` | - | Filter by type, for example whisper, vad, punctuation |
| `--language` | `-l` | - | Filter by language token, for example zh, en, ja, yue |
| `--recommended` | `-r` | - | Only include recommended preset models |
| `--installed` | `-i` | - | Only include models already present in the models directory |
| `--all` | `-a` | - | Include auxiliary companion models (VAD, punctuation, speaker embedding) |
| `--json` | `-j` | - | Print machine-readable JSON |
| `<QUERY>` | - | - | Filter models by keyword matching ID, alias, or type |
| `--online` | - | - | Show supported online cloud providers alongside local models |

**Usage & Examples:**

```text
Examples:
  sona-cli models list
  sona-cli models list --mode batch --type whisper
  sona-cli models list --language zh --installed
```

##### `sona-cli models path`

Prints the resolved models directory path

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--models-dir` | - | - | Override the target models directory |

**Usage & Examples:**

```text
Examples:
  sona-cli models path
  sona-cli models path --models-dir ./models
```

##### `sona-cli models verify`

Verifies the integrity of an installed preset model

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<MODEL_ID>` | - | - | Preset model id, for example whisper-turbo or silero-vad (defaults to all installed models when omitted) |
| `--models-dir` | - | - | Override the models directory |
| `--all` | - | - | Verify all installed models in the models directory |

**Usage & Examples:**

```text
Examples:
  sona-cli models verify whisper-turbo
  sona-cli models verify sherpa-onnx-whisper-turbo --models-dir ./models
```

#### `sona-cli providers`

Lists supported online ASR providers

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<PROVIDER>` | - | - | Optional provider ID to inspect in detail |
| `--models` | `-m` | - | Show supported models for each provider |
| `--json` | `-j` | - | Print machine-readable JSON |

**Usage & Examples:**

```text
Examples:
  sona-cli providers
  sona-cli providers --models
  sona-cli providers groq-whisper
  sona-cli providers --json
```

#### `sona-cli serve`

Runs the shared local HTTP API server

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--config` | `-c` | - | Optional config file, usually sona-cli.toml |
| `--host` | - | - | Host/IP address to bind [default: 127.0.0.1] |
| `--port` | `-p` | - | TCP port to bind [default: 14200] |
| `--api-key` | - | - | Bearer token required for private endpoints |
| `--models-dir` | - | - | Models directory containing installed presets |
| `--ip-whitelist` | - | - | Comma-separated IP whitelist, for example localhost,192.168.1.0/24 |
| `--max-streaming` | - | - | Maximum concurrent streaming sessions |
| `--max-concurrent` | - | - | Maximum concurrent transcription jobs |
| `--max-queue-size` | - | - | Maximum queued transcription jobs; 0 means effectively unbounded |
| `--max-upload-size-mb` | - | - | Maximum upload size in MiB; 0 disables the request body limit |
| `--job-ttl-minutes` | - | - | Completed job retention window in minutes; 0 disables cleanup |
| `--gpu-acceleration` | - | - | GPU acceleration mode: auto, cpu, vulkan, metal, or cuda |
| `--vad-model-id` | - | - | VAD model id override |
| `--punctuation-model-id` | - | - | Punctuation model id override |
| `--ffmpeg-path` | - | - | Custom path to the ffmpeg executable |

**Usage & Examples:**

```text
Core Endpoints:
  GET  /health                  Health check and status
  GET  /info                    Server info, model list, and capabilities
  POST /v1/transcriptions       Sona native batch transcription
  POST /v1/audio/transcriptions OpenAI-compatible audio transcription
  GET  /v1/transcriptions/jobs  List transcription jobs
  WS   /v1/streaming              Real-time streaming

Authentication:
  When --api-key is set, provide header 'Authorization: Bearer <API_KEY>'.

Examples:
  sona-cli serve
  sona-cli serve --host 127.0.0.1 --port 14200
  sona-cli serve --api-key my-secret-token
  sona-cli serve --config ./sona-cli.toml
```

#### `sona-cli transcribe`

Transcribe audio with local or online ASR; local ASR also accepts video

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<INPUT>` | - | - | Input audio file(s), video file(s), or glob pattern(s). Required unless --input-dir is specified |
| `--input-dir` | - | - | Directory containing input files for batch transcription |
| `--output-dir` | - | - | Directory to write transcript files for batch transcription |
| `--recursive` | - | `false` | Recursively scan input directory |
| `--output` | `-o` | - | Output transcript file. Defaults to stdout when omitted (single-input mode only) |
| `--format` | `-f` | - | Export format: json, txt, srt, vtt, or md |
| `--mode` | - | `original` | Text selection mode: original, translation, or bilingual |
| `--force` | `-F` | `false` | Overwrite existing output files |
| `--continue-on-error` | - | `false` | Continue processing remaining files if an error occurs during batch transcription |
| `--model` | `-m` | - | Preset model id to use, or online model name when an online provider is selected |
| `--models-dir` | - | - | Models directory containing installed presets |
| `--language` | `-l` | - | Override the language setting |
| `--vad-model-id` | - | - | VAD model id override |
| `--punctuation-model-id` | - | - | Punctuation model id override |
| `--online-provider` | - | - | Use an online ASR provider instead of local Sherpa ASR (alias: --provider) |
| `--api-key` | - | - | Direct API key for the online ASR provider. Takes precedence over --api-key-env |
| `--api-key-env` | - | - | Environment variable containing the online ASR API key. Default env vars: volcengine-doubao: SONA_VOLCENGINE_ASR_API_KEY, groq-whisper: GROQ_API_KEY, mistral-voxtral: MISTRAL_API_KEY, openai-whisper: OPENAI_API_KEY, deepgram: DEEPGRAM_API_KEY, assemblyai: ASSEMBLYAI_API_KEY, elevenlabs: ELEVENLABS_API_KEY |
| `--online-model` | - | - | Override the model name for the online ASR provider (e.g. whisper-large-v3, nova-2) |
| `--online-param` | - | - | Pass arbitrary KEY=VALUE configuration options to the online provider (can be repeated) |
| `--online-config` | - | - | JSON object overriding non-secret provider endpoint or model settings |
| `--threads` | - | - | Number of recognition threads (default: system auto-configured) |
| `--enable-itn` | - | `false` | Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120") |
| `--hotwords` | - | - | Optional hotwords string to enhance recognition, separated by newlines or commas |
| `--gpu-acceleration` | - | - | GPU acceleration mode: auto, cpu, vulkan, metal, or cuda |
| `--vad-buffer` | - | - | VAD buffer size in seconds, for example 0.5 |
| `--save-wav` | - | - | Save the resampled WAV to a file |
| `--ffmpeg-path` | - | - | Custom path to the ffmpeg executable |
| `--config` | `-c` | - | Optional config file, usually sona-cli.toml |
| `--quiet` | `-q` | `false` | Suppress progress output |

**Usage & Examples:**

```text
Examples:
  sona-cli transcribe ./sample.wav -m whisper-turbo
  sona-cli transcribe ./sample.wav -m sensevoice -l zh -f txt
  sona-cli transcribe ./sample.wav -m whisper-turbo -o ./out.srt
  sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts -f srt
  sona-cli transcribe ./sample.wav --online-provider groq-whisper --output ./out.srt
  sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --api-key-env MY_ASR_KEY
```

## Internal Integration Contracts (Hidden Commands)

The following commands are hidden from `--help` by default and are intended for host/desktop integration, facts snapshot construction, and automated path contract inspection:

### `sona-cli diagnostics`

Builds diagnostics snapshots from host-provided facts

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--app-data-dir` | - | - | Application data directory containing the models directory |
| `--input` | - | - | JSON file containing host diagnostics facts and model paths |
| `--json` | - | `false` | Prints JSON instead of the default table output |

**Usage & Examples:**

```text
Examples:
  sona-cli diagnostics --app-data-dir ./app_data --input ./facts.json
  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json
```

#### `sona-cli diagnostics snapshot`

Builds a diagnostics snapshot from host-provided facts

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `--app-data-dir` | - | - | Application data directory containing the models directory |
| `--input` | - | - | JSON file containing host diagnostics facts and model paths |
| `--json` | - | - | Prints JSON instead of the default table output |

**Usage & Examples:**

```text
Input JSON format (DiagnosticsCoreInput):
  {
    "config": {
      "streamingModelPath": "/path/to/streaming-model",
      "batchModelPath": "/path/to/batch-model",
      "vadModelPath": "",
      "punctuationModelPath": "",
      "microphoneId": "default"
    },
    "permissionState": "granted",
    "microphoneProbe": { "options": [], "available": true, "errorMessage": null },
    "systemAudioProbe": { "options": [], "available": false, "errorMessage": null },
    "voiceTypingReadiness": { "state": "ready", "lastErrorMessage": null }
  }

Examples:
  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json
  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json --json
```

### `sona-cli path-status`

Resolves a filesystem path using the shared runtime status contract

**Options & Arguments:**

| Option / Argument | Short | Default | Description |
| --- | --- | --- | --- |
| `<PATH>` | - | - | Filesystem path to inspect |
