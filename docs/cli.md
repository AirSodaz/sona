# Sona CLI

`sona-cli` is Sona's command line entry point for offline batch transcription. Release installers and app bundles include it, and source builds can still run it directly with Cargo.

The current CLI covers two core workflows:

- Transcribe one local audio or video file
- List available preset models and download models
- Export the result as `json`, `txt`, `srt`, or `vtt`
- Reuse the same preset model metadata as the desktop app

## Current Scope

- Included in desktop installers and app bundles
- Supported inside `src-tauri/`: `cargo run --bin sona-cli -- ...`
- Supported inside `src-tauri/`: `cargo build --release --bin sona-cli`
- Not included yet: shell `PATH` registration, live recording, LLM polish, LLM translate

## Installed Locations

`sona-cli` is packaged with the desktop app, but it is not registered on `PATH`.

- Windows: run `sona-cli.exe` from the same installation directory as `Sona.exe`
- macOS: run `/Applications/Sona.app/Contents/Resources/sona-cli`
- Linux packages: run `sona-cli` from the Tauri resource directory, typically `/usr/lib/Sona/sona-cli`
- AppImage: run `sona-cli` from the mounted AppImage resource directory, typically `${APPDIR}/usr/lib/Sona/sona-cli`

## Common Commands

### Transcribe a file

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Without `--output`, `sona-cli` writes JSON to `stdout`.

### List models

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models list
```

Common filters:

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models list --mode offline --type whisper
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models list --language zh --installed
```

### Download a model

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models download sherpa-onnx-whisper-turbo
```

If the preset requires companion models, the CLI downloads them automatically:

- `silero-vad` for presets that require VAD
- The default punctuation preset for presets that require punctuation

You can also override the models directory explicitly:

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models download silero-vad --models-dir ./models
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
sona-cli transcribe <input>
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
sona-cli models list
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
sona-cli models download <model_id>
  --models-dir <path>
  --quiet
```

## Help Commands

```bash
sona-cli --help
sona-cli transcribe --help
sona-cli models --help
sona-cli models list --help
sona-cli models download --help
```

## Manual Verification

### Verify model listing

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models list --type whisper --language zh
```

Expected result:

- JSON is printed to `stdout`
- Only models matching the filters are included

### Verify model download

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- models download sherpa-onnx-whisper-turbo
```

Expected result:

- Download progress is printed to `stderr`
- The main model is installed into the models directory
- Required VAD and punctuation companions are installed automatically

### Verify transcription

With locally installed desktop models, verify the CLI manually. This works for both source builds and packaged installs as long as `models_dir` points at the desktop model directory:

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Expected result:

- Progress is printed to `stderr`
- The target export file is created
- The selected preset model and companion model ids resolve under `models_dir`
