# Sona CLI

`sona` supports offline batch transcription commands directly from the main desktop executable. Packaged installs expose CLI subcommands through the app binary, and source builds can run the same commands with Cargo.

The current CLI covers two core workflows:

- Transcribe one local audio or video file
- List available preset models and download models
- Export the result as `json`, `txt`, `srt`, or `vtt`
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
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Without `--output`, `sona` writes JSON to `stdout`.

### List models

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list
```

Common filters:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list --mode offline --type whisper
cargo run --manifest-path src-tauri/Cargo.toml -- models list --language zh --installed
```

### Download a model

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download sherpa-onnx-whisper-turbo
```

If the preset requires companion models, the CLI downloads them automatically:

- `silero-vad` for presets that require VAD
- The default punctuation preset for presets that require punctuation

You can also override the models directory explicitly:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download silero-vad --models-dir ./models
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
- `itn_model_ids`
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

### `transcribe`

```text
sona transcribe <input>
  --config <path>
  --output <path>
  --format <json|txt|srt|vtt>
  --language <code>
  --model-id <id>
  --models-dir <path>
  --vad-model-id <id>
  --punctuation-model-id <id>
  --itn-model-id <id>    # repeatable
  --threads <n>
  --enable-itn
  --disable-itn
  --vad-buffer <seconds>
  --save-wav <path>
  --quiet
```

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
sona transcribe --help
sona models --help
sona models list --help
sona models download --help
```

## Manual Verification

### Verify model listing

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models list --type whisper --language zh
```

Expected result:

- JSON is printed to `stdout`
- Only models matching the filters are included

### Verify model download

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- models download sherpa-onnx-whisper-turbo
```

Expected result:

- Download progress is printed to `stderr`
- The main model is installed into the models directory
- Required VAD and punctuation companions are installed automatically

### Verify transcription

With locally installed desktop models, verify the CLI manually. This works for both source builds and packaged installs as long as `models_dir` points at the desktop model directory:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Expected result:

- Progress is printed to `stderr`
- The target export file is created
- The selected preset model and companion model ids resolve under `models_dir`
