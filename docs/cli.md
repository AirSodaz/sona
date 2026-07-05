# Sona CLI

`sona-cli` is the standalone command line interface for Sona. The desktop Tauri app no longer embeds CLI subcommands, so packaged desktop builds should launch `sona-cli` for command-line workflows.

This document tracks the commands that ship in the standalone CLI today:

- `path-status`
- `init-config`
- `models list`
- `models download`
- `models delete`

Offline transcription and headless API server commands are being migrated into `sona-cli` and are not part of the current standalone surface yet.

## Run It

- Packaged builds: use the `sona-cli` binary bundled with the same-platform installer output
- Source builds: `cargo run -p sona-cli -- <command> ...`

Examples:

```bash
cargo run -p sona-cli -- path-status .
cargo run -p sona-cli -- init-config
cargo run -p sona-cli -- models list --json
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

## Global Flags

```text
sona-cli
  -V, --version
  -h, --help
```

Run `sona-cli <command> --help` for command-specific usage.
