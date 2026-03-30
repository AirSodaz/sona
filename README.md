# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** is a powerful, offline transcript editor built with [Tauri](https://tauri.app), [React](https://react.dev), and [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). It provides fast, accurate, and private speech-to-text capabilities directly on your local machine using a high-performance Rust backend.

## ✨ Features

- **🔒 Offline & Private**: All speech processing happens locally on your device. No data leaves your machine.
- **🎙️ Real-time Transcription**: Record and transcribe audio in real-time with low latency.
- **📁 Batch Processing**: Import multiple audio or video files for bulk transcription in the background.
- **📝 Interactive Editor**: A rich text editor synchronized with audio playback for easy corrections.
- **✨ LLM Assistant**: Polish and translate transcripts using OpenAI, Anthropic, Gemini, or Ollama.
- **📤 Smart Export**: Export in multiple formats (TXT, SRT, VTT, JSON) with bilingual support.
- **🤖 Advanced AI Models**: Powered by state-of-the-art models like **SenseVoice**, **Whisper**, and **Paraformer**.

## 🚀 Getting Started

### Download from GitHub Releases

The easiest way to install Sona is to download the pre-built binaries for your platform from the [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) page.

### User Guide

For end-user setup and daily workflows, read the [User Guide](docs/user-guide.md). It covers first-run setup, `Live Record`, `Batch Import`, transcript editing, LLM features, export, history, and troubleshooting.

### CLI

Sona supports offline batch transcription commands directly through the main desktop executable. Packaged installs do not add it to your shell `PATH`, so invoke the app binary itself with CLI subcommands.

Installed package locations:

- Windows: run `Sona.exe transcribe ...` from the installation directory
- macOS: run `/Applications/Sona.app/Contents/MacOS/Sona transcribe ...`
- Linux: run the packaged `Sona` binary with CLI subcommands from the install location
- AppImage: run the mounted AppImage executable with CLI subcommands

Source builds can still run the CLI directly with Cargo:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- transcribe ./sample.mp4 --config ./sona-cli.toml --output ./sample.srt
```

Current CLI scope is intentionally narrow:

- Single-file offline transcription
- Export to `json`, `txt`, `srt`, or `vtt`
- Exposed through the main desktop executable, but not registered on `PATH`

For the full CLI guide and a minimal TOML example, read [docs/cli.md](docs/cli.md).

### Build from Source

#### Prerequisites

*   **Node.js**: v20 or later (for frontend build).
*   **Rust**: Stable release (required for the Tauri backend).
*   **Package Manager**: `npm` (recommended).

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
    *(Note: This also triggers the existing `scripts/setup-ffmpeg.js` prebuild/setup step for local development.)*
    ```bash
    npm install
    ```

3.  **Run the application**
    ```bash
    npm run tauri dev
    ```

4.  **Run frontend tests**
    ```bash
    npm test
    ```

## 📦 Model Management

Sona allows you to choose the AI model that best fits your needs, both for offline transcription and online assistance.

### Offline Transcription
1.  Navigate to **Settings > Models**.
2.  Choose from a curated list of high-performance models:
    *   **SenseVoice**: Best for multilingual support and emotion recognition.
    *   **Whisper (Tiny)**: Lightweight version of OpenAI's Whisper model.
    *   **Paraformer**: Optimized for streaming.
3.  Click **Download**. The model will be automatically stored locally.

### LLM Assistant (Polish & Translate)
1.  Navigate to **Settings > LLM Service**.
2.  Select your provider (OpenAI, Anthropic, Gemini, or Ollama).
3.  Enter your API Key and Base URL (if applicable).
4.  Select a model to power the Polish and Translate features.

## 🏗️ Building

To build the application for production:

```bash
npm run tauri build
```

The executable will be generated in `src-tauri/target/release/bundle`.
