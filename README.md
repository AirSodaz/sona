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

## 📚 Documentation

Choose the guide that matches what you want to do:

| Resource | What it covers | Languages |
| --- | --- | --- |
| User Guide | Installation, first-run setup, transcription, editing, and export | [English](docs/user-guide.md) · [简体中文](docs/user-guide.zh-CN.md) |
| CLI Guide | Standalone commands, automation, live transcription, and configuration | [English](docs/cli.md) · [简体中文](docs/cli.zh-CN.md) |
| HTTP API Reference | Server configuration, authentication, endpoints, and webhooks | [English](docs/api.md) · [简体中文](docs/api.zh-CN.md) |
| Development Guide | Local setup, testing, desktop builds, and CLI builds | [English](docs/development.md) · [简体中文](docs/development.zh-CN.md) |
| Nightly Workflow | Nightly triggers, build jobs, artifacts, and publishing | [English](docs/nightly-workflow.md) · [简体中文](docs/nightly-workflow.zh-CN.md) |
| Contributing | Branches, validation, commits, and pull request expectations | [English](CONTRIBUTING.md) |
