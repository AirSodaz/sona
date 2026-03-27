# Sona CLI

`sona-cli` is Sona's command line entry point for offline batch transcription. Release installers and app bundles include it, and source builds can still run it directly with Cargo.

This v1 implementation focuses on a single workflow:

- Transcribe one local audio or video file
- Export the result as `json`, `txt`, `srt`, or `vtt`
- Reuse the same installed preset model metadata as the desktop app

## Current Scope

- Included in desktop installers and app bundles
- Supported inside `src-tauri/`: `cargo run --bin sona-cli -- ...`
- Supported inside `src-tauri/`: `cargo build --release --bin sona-cli`
- Not included yet: shell `PATH` registration, standalone CLI-only archives, live recording, AI polish, AI translate

## Installed Locations

`sona-cli` is packaged with the desktop app, but it is not registered on `PATH`.

- Windows: run `sona-cli.exe` from the same installation directory as `Sona.exe`
- macOS: run `/Applications/Sona.app/Contents/Resources/sona-cli`
- Linux packages: run `sona-cli` from the Tauri resource directory, typically `/usr/lib/Sona/sona-cli`
- AppImage: run `sona-cli` from the mounted AppImage resource directory, typically `${APPDIR}/usr/lib/Sona/sona-cli`

## Example

```bash
cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 \
  --config ./sona-cli.toml \
  --output ./sample.srt
```

Without `--output`, `sona-cli` writes JSON to `stdout`.

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

- If the selected offline model requires VAD, pass `--vad-model-id` or set `vad_model_id` in the config file.
- If the selected offline model requires punctuation, pass `--punctuation-model-id` or set `punctuation_model_id` in the config file.

The CLI does not auto-select companion models. Missing required companion models fail fast with an error.

## Common Flags

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

## Manual Verification

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
