# Sona CLI

`sona` supports offline batch transcription commands directly from the main desktop executable. Packaged installs expose CLI subcommands through the app binary, and source builds can run the same commands with Cargo.

The current CLI covers three core workflows:

- Transcribe one local audio or video file
- List available preset models and download models
- Start a headless HTTP API server for remote integration
- Export the result as `json`, `txt`, `srt`, `vtt`, or `md`
- Reuse the same preset model metadata as the desktop app

## Current Scope

- Included in desktop installers and app bundles through the main executable
- Supported from source with `cargo run -- ...`
- Not included yet: shell `PATH` registration, live recording, LLM polish, LLM translate

## Installed Locations

The CLI is available through the packaged app binary, but it is not registered on `PATH`.

- Windows: run `Sona.exe transcribe ...` from the same installation directory
- macOS: run `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux packages: run the packaged `Sona` binary with CLI subcommands from the install location
- AppImage: run the mounted AppImage executable with CLI subcommands

## Common Commands

### Transcribe a file

```bash
sona transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Without `--output`, `sona` writes JSON to `stdout`.

### List models

```bash
sona models list
```

Common filters:

```bash
sona models list --mode offline --type whisper
sona models list --language zh --installed
```

### Download a model

```bash
sona models download sherpa-onnx-whisper-turbo
```

If the preset requires companion models, the CLI downloads them automatically:

- `silero-vad` for presets that require VAD
- The default punctuation preset for presets that require punctuation

You can also override the models directory explicitly:

```bash
sona models download silero-vad --models-dir ./models
```

## Config File

Pass a config file explicitly with `--config`.

Minimal example:

```toml
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
format = "srt"
```

Supported keys:

- `models_dir`
- `model_id`
- `vad_model_id`
- `punctuation_model_id`
- `language`
- `threads`
- `enable_itn`
- `vad_buffer_size`
- `format`

Command-line flags override config file values.

## Required Companion Models

Some offline models require companion assets:

- If the selected offline model requires VAD, you must provide a valid `vad_model_id`
- If the selected offline model requires punctuation, you must provide a valid `punctuation_model_id`

For `models download`:

- The CLI automatically downloads required VAD and punctuation companions

For `transcribe`:

- You still need to pass `--vad-model-id`, `--punctuation-model-id`, or define them in the config file
- Missing required companion ids fail fast with an error

## Common Flags

### Global

```text
sona
  -V, --version
  -v, --verbose
```

Use `-V` or `--version` to print the Sona version. Use `-v` or `--verbose` before a subcommand to enable detailed diagnostic logs:

```bash
sona --version
sona -V
sona -v models list
sona --verbose transcribe ./sample.mp4 --config ./sona-cli.toml
```

Verbose diagnostics are written to `stderr`. Command output, including JSON output from `models list` and `transcribe` without `--output`, remains on `stdout` so it can still be piped to other tools.

### `transcribe`

```text
sona transcribe <input>
  --config <path>
  --output <path>
  --format <json|txt|srt|vtt|md>
  --language <code>
  --model-id <id>
  --models-dir <path>
  --vad-model-id <id>
  --punctuation-model-id <id>
  --threads <n>
  --enable-itn
  --disable-itn
  --vad-buffer <seconds>
  --save-wav <path>
  --quiet
```

### `serve`

```text
sona serve
  --host <ip>
  --port <port>
  --api-key <key>
  --models-dir <path>
  --ip-whitelist <rules>
  --max-streaming <n>
```

For more details on API usage, see [api.md](api.md).

### `models list`

```text
sona models list
  --models-dir <path>
  --mode <streaming|offline>
  --type <type>
  --language <code>
  --installed
```

Notes:

- `--type` can be values like `whisper`, `vad`, or `punctuation`
- `--language` matches language tokens like `zh`, `en`, `ja`, or `yue`
- The current default output format is JSON

### `models download`

```text
sona models download <model_id>
  --models-dir <path>
  --quiet
```

## Help Commands

```bash
sona --help
sona help
sona --version
sona -V
sona transcribe --help
sona models --help
sona models list --help
sona models download --help
sona serve --help
```
