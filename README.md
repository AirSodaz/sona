# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** is a powerful, offline transcript editor built with [Tauri](https://tauri.app), [React](https://react.dev), and [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). It provides fast, accurate, and private speech-to-text capabilities directly on your local machine using a high-performance Rust backend.

## ✨ Features

- **🔒 Offline & Private**: All speech processing happens locally on your device. No data leaves your machine.
- **🎙️ Real-time Transcription**: Record and transcribe audio in real-time with low latency.
- **📁 Batch Processing**: Import multiple audio or video files for bulk transcription in the background.
- **🗂️ Workspace Organization**: Use `Workspace`, `Projects`, and `Inbox` to organize saved recordings and imports.
- **📝 Interactive Editor**: A rich text editor synchronized with audio playback for corrections, speaker labels, and version snapshots.
- **👥 Speaker Profiles & Review**: Build local speaker profiles, correct speaker badges segment by segment, and review suggested or anonymous speaker groups before export.
- **✨ LLM Assistant**: Polish, translate, and summarize transcripts using OpenAI, Anthropic, Gemini, or Ollama.
- **🗣️ Live Caption & Voice Typing**: Reuse the same offline live transcription stack for floating captions or dictation into other applications.
- **📤 Smart Export**: Export in multiple formats (TXT, SRT, VTT, JSON) with bilingual support.
- **🛟 Recovery, Backup & Diagnostics**: Resume interrupted work, export lightweight backups of config, workspace, and text history, and inspect model/runtime health from the app.
- **🔔 Notifications & Automation**: Use the header notification center for updates, recovery, and automation results, and configure watched-folder automation rules in Settings.
- **🤖 Advanced AI Models**: Powered by state-of-the-art models like **SenseVoice**, **Whisper**, and **Paraformer**.

## 🚀 Getting Started

### Download from GitHub Releases

The easiest way to install Sona is to download the pre-built binaries for your platform from the [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) page.

### User Guide

For end-user setup and daily workflows, read the [User Guide](docs/user-guide.md). It covers first-run setup, `Live Record`, `Batch Import`, `Workspace` / `Projects` / `Inbox`, transcript editing, speaker review, version snapshots, LLM features, `Voice Typing`, export, `Dashboard` / backup / recovery entry points, and troubleshooting.

### CLI

Sona ships command-line workflows through the standalone `sona-cli` binary. The desktop Tauri app no longer parses CLI subcommands; release and nightly builds stage `sona-cli` into the same-platform desktop installer resources so it can share the bundled Sherpa-onnx dynamic libraries.

Installed package notes:

- Windows/macOS/Linux packages include the matching `sona-cli` resource beside the desktop bundle resources.
- `sona-cli` is independent from the desktop executable and can also be built as its own release binary.

Source builds can run or build the CLI directly from the workspace:

```bash
cargo run -p sona-cli -- transcribe ./sample.mp4 -c ./sona-cli.toml --output ./sample.srt
cargo run -p sona-cli -- serve --host 127.0.0.1 --port 14200
pnpm run build:sona-cli
```

Current standalone CLI scope:

- Single-file offline transcription
- Preset model listing, downloads, and deletion
- Shared local HTTP API server via `sona-cli serve`
- Runtime path status inspection
- Commented `sona-cli.toml` starter generation

For the full CLI guide and the `sona-cli init-config` TOML template workflow, read [docs/cli.md](docs/cli.md).

### Build from Source

#### Prerequisites

*   **Node.js**: v20 or later (for frontend build).
*   **Rust**: Stable release (required for the Tauri backend).
*   **Package Manager**: `pnpm` via Corepack (recommended).

##### Linux Requirements
If you are running on Linux (Ubuntu/Debian), ensure you have the necessary system dependencies:

```bash
sudo apt-get update
sudo apt-get install libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libasound2-dev
```

#### Installation

1.  **Clone the repository**
    ```bash
    git clone https://github.com/AirSodaz/sona.git
    cd sona
    ```

2.  **Install dependencies**
    ```bash
    corepack enable
    pnpm install
    ```

3.  **Run the application**
    ```bash
    pnpm run tauri dev
    ```

4.  **Run frontend tests**
    ```bash
    pnpm test
    ```

## 📦 Model Management

Sona allows you to choose the AI model that best fits your needs, both for offline transcription and online assistance.

### Offline Transcription
1.  Navigate to **Settings > Model Settings**.
2.  Choose from a curated list of high-performance models:
    *   **SenseVoice**: Best for multilingual support and emotion recognition.
    *   **Whisper (Tiny)**: Lightweight version of OpenAI's Whisper model.
    *   **Paraformer**: Optimized for streaming.
3.  Click **Download**. The model will be automatically stored locally.

### LLM Assistant (Polish, Translate, Summary)
1.  Navigate to **Settings > LLM Service**.
2.  Select your provider (OpenAI, Anthropic, Gemini, or Ollama).
3.  Enter your API Key and Base URL (if applicable).
4.  Select the models that power polish, translation, and summary generation.

## 🏗️ Building

To build the application for production:

```bash
pnpm run tauri build
```

Desktop bundles are generated under `target/release/bundle` or `target/<triple>/release/bundle` depending on the build target.
