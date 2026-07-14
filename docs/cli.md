# Sona CLI

`sona-cli` is the standalone command line interface for Sona. The desktop Tauri app no longer embeds CLI subcommands, so packaged desktop builds should launch `sona-cli` for command-line workflows.

This document tracks the commands that ship in the standalone CLI today:

- `path-status`
- `init-config`
- `models list`
- `models download`
- `models delete`
- `history list|query|transcript|snapshots|snapshot|<mutation>`
- `export transcript`
- `backup export|inspect|import`
- `serve`
- `transcribe`
- `transcribe-live`

The headless HTTP API server is backed by the shared `sona-api-server` adapter and can be started from either the desktop app or `sona-cli serve`.

## Run It

- Packaged builds: use the `sona-cli` binary bundled with the same-platform installer output
- Source builds: `cargo run -p sona-cli -- <command> ...`

Examples:

```bash
cargo run -p sona-cli -- path-status .
cargo run -p sona-cli -- init-config
cargo run -p sona-cli -- models list --json
cargo run -p sona-cli -- history list --app-data-dir ./sona-data --json
cargo run -p sona-cli -- export transcript --input ./segments.json --output ./transcript.vtt
cargo run -p sona-cli -- serve --host 127.0.0.1 --port 14200
cargo run -p sona-cli -- transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
cargo run -p sona-cli -- transcribe-live --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17
```

## Commands

### `path-status`

Resolve one filesystem path through the shared runtime status contract and print JSON to `stdout`.

```bash
sona-cli path-status ./models
```

Example output:

```json
{
  "kind": "directory",
  "path": "C:/work/models",
  "error": null
}
```

### `init-config`

Create a commented starter config file for future standalone CLI workflows.

```bash
sona-cli init-config
sona-cli init-config ./sona-cli.toml --force
```

- Default output path: `./sona-cli.toml`
- Existing files are protected unless `--force` is passed
- Status text is written to `stderr`

### `models list`

List preset models, optionally filtered by mode, type, language, or installation state.

```bash
sona-cli models list
sona-cli models list --mode offline --type whisper
sona-cli models list --language zh --installed
sona-cli models list --json
```

`--json` prints the full machine-readable shape, including `install_path`.

### `models download`

Download a preset model into the resolved models directory.

```bash
sona-cli models download sherpa-onnx-whisper-turbo
sona-cli models download silero-vad --models-dir ./models --yes
```

When the chosen preset requires companion assets, `sona-cli` also downloads the required VAD or punctuation model.

### `models delete`

Delete one installed preset model.

```bash
sona-cli models delete sherpa-onnx-whisper-turbo --yes
sona-cli models delete silero-vad --models-dir ./models --yes
```

Companion models are not deleted automatically.

### `history`

Query or mutate an existing Sona application data directory through the shared history services.

```bash
sona-cli history list --app-data-dir ./sona-data --limit 50 --offset 0
sona-cli history query --app-data-dir ./sona-data --input ./workspace-query.json --json
sona-cli history transcript --app-data-dir ./sona-data --history-id <ID> --json
sona-cli history snapshots --app-data-dir ./sona-data --history-id <ID>
sona-cli history snapshot --app-data-dir ./sona-data --history-id <ID> --snapshot-id <ID> --json
sona-cli history create-live-draft --app-data-dir ./sona-data --id <ID> --audio-extension wav --json
sona-cli history complete-live-draft --app-data-dir ./sona-data --history-id <ID> --segments ./segments.json --duration 12.5 --json
sona-cli history save-recording --app-data-dir ./sona-data --input ./recording.json --audio ./recording.wav --json
sona-cli history import-file --app-data-dir ./sona-data --input ./history-import.json --json
sona-cli history update-transcript --app-data-dir ./sona-data --history-id <ID> --segments ./segments.json --json
sona-cli history create-snapshot --app-data-dir ./sona-data --history-id <ID> --reason polish --segments ./segments.json --json
sona-cli history update-meta --app-data-dir ./sona-data --history-id <ID> --updates ./history-meta.json
sona-cli history assign-project --app-data-dir ./sona-data --history-id <ID> --project-id <PROJECT_ID>
sona-cli history reassign-project --app-data-dir ./sona-data --current-project-id <PROJECT_ID>
sona-cli history delete --app-data-dir ./sona-data --history-id <ID>
```

- `query` accepts the same camelCase `HistoryWorkspaceQueryRequest` JSON contract as Tauri and UniFFI.
- `list`, `query`, and `snapshots` use tables by default; `--json` preserves the complete machine-readable response.
- `transcript` and `snapshot` display segment tables by default.
- `recording.json` contains `segments`, `duration`, optional `projectId`, and optional `audioExtension`; audio bytes are read only from `--audio`.
- `history-import.json` uses the camelCase `HistorySaveImportedFileRequest` contract. `--segments` files contain a JSON array and `--updates` contains a JSON object.
- Omit the target project option on `assign-project` or `reassign-project` to move records to the inbox.
- The application data directory must already exist. Invalid mutation input is rejected before the lazy SQLite adapter opens the database.

### `export transcript`

Export an existing JSON array of transcript segments through the shared core export service.

```bash
sona-cli export transcript --input ./segments.json --output ./transcript.vtt
sona-cli export transcript --input ./segments.json --output ./transcript.srt --mode bilingual
sona-cli export transcript --input ./segments.json --output ./transcript.txt --format txt --json
```

- The format is inferred from the output extension unless `--format` is provided.
- Supported formats: `json`, `txt`, `srt`, `vtt`, `md`.
- Supported modes: `original` (default), `translation`, `bilingual`.
- `--json` prints the output path and written byte count as machine-readable JSON.

### `backup`

Export all five supported application-state scopes, validate an archive without opening application data, or atomically replace the backed-up state from an archive.

```bash
sona-cli backup export --app-data-dir ./sona-data --output ./sona-backup.sona-backup --app-version 0.8.0
sona-cli backup inspect --archive ./sona-backup.sona-backup
sona-cli backup import --app-data-dir ./sona-data --archive ./sona-backup.sona-backup --default-rule-set-name "Default Rules" --confirm-replace
```

Import atomically replaces the `config`, `workspace`, `history`, `automation`, and `analytics` scopes. It requires `--confirm-replace` and never opens an interactive prompt. Task ledger and original audio are not included in backup archives.

### `serve`

Run the shared local HTTP API server from the standalone CLI.

```bash
sona-cli serve
sona-cli serve --config ./sona-cli.toml
sona-cli serve --host 127.0.0.1 --port 14200 --api-key local-secret
```

- Runs until Ctrl+C
- Reuses the same local batch transcription API server adapter as the desktop app
- `--config` reads the `[serve]` section generated by `init-config`
- Local REST transcription works from the CLI. The `serve` WebSocket streaming router and online ASR integrations still require the desktop runtime; use `transcribe-live` for standalone local streaming.

### `transcribe`

Transcribe one local audio or video file with the offline ASR adapter.

```bash
sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo
sona-cli transcribe ./sample.wav --config ./sona-cli.toml --output ./out.srt
sona-cli transcribe ./sample.wav --format txt --quiet
```

- Output defaults to `stdout` when `--output` is omitted
- Supported export formats: `json`, `txt`, `srt`, `vtt`, `md`
- `--config` reads the commented `sona-cli.toml` template generated by `init-config`
- `--force` allows overwriting an existing output file

### `transcribe-live`

Transcribe microphone or raw stdin audio in real time with an installed local streaming model.

```bash
sona-cli transcribe-live --list-input-devices
sona-cli transcribe-live \
  --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17 \
  --device "Studio Mic" \
  --duration 60 \
  --output ./live.srt
ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \
  sona-cli transcribe-live \
    --input stdin \
    --model-id sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en \
    --output-format ndjson
```

- The command supports local offline ASR only. The selected preset must declare `streaming` mode and be installed in the resolved models directory.
- Microphone input uses the default CPAL input device unless `--device` supplies an exact name returned by `--list-input-devices`.
- `--input stdin` accepts headerless 16 kHz, mono, signed 16-bit little-endian PCM. An incomplete final sample is an input error.
- TTY text output refreshes the current transcript in place. Redirected text output writes one final snapshot after flushing.
- `--output-format ndjson` emits one JSON object per line and flushes every event. Event types are `started`, `update`, `stopped`, and runtime-only `error`; transcript fields use camelCase.
- Ctrl+C, stdin EOF, and `--duration` perform the same graceful sequence: drain capture audio, flush and stop ASR, optionally write the final file, emit `stopped`, and exit 0.
- `--output` writes the final snapshot as `json`, `txt`, `srt`, `vtt`, or `md`. The extension selects the format unless `--format` is present; `--force` is required to replace an existing file.
- `--config` reads `[transcribe_live]` from `sona-cli.toml`. Command-line values override the section, which inherits shared top-level ASR defaults. Final output path, export format, and overwrite behavior remain command-line-only.
- Validation errors exit 2, model errors exit 3, and input/device errors exit 5. Runtime NDJSON errors are also written as an `error` event before the non-zero exit.

## Global Flags

```text
sona-cli
  -V, --version
  -h, --help
```

Run `sona-cli <command> --help` for command-specific usage.
