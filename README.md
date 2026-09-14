# Sona

[English](README.md) | [简体中文](README.zh-CN.md)

**Sona** is a powerful, offline transcript editor built with [Tauri](https://tauri.app), [React](https://react.dev), [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), and [llama.cpp](https://github.com/ggerganov/llama.cpp). It provides fast, accurate, and private speech-to-text capabilities directly on your local machine using a high-performance Rust backend.

## ✨ Features

- **🔒 Offline & Private**: Speech processing happens locally on your device. Audio recordings never leave your machine.
- **🎙️ Real-time Transcription**: Record and transcribe audio in real time with low latency.
- **📁 Batch Processing**: Import audio or video files for background batch transcription with real-time cancellation support.
- **☁️ End-to-End Encrypted Cloud Sync**: Multi-device sync via WebDAV with client-side end-to-end encryption (E2EE), master password, emergency recovery keys, configurable sync scopes (Content, Standard, Full), and conflict resolution.
- **📱 Cross-Platform & Android**: Core application capabilities available on desktop (Windows, macOS, Linux) and Android via UniFFI.
- **🗂️ Project Center & Automated Pipelines**: Organize work with `Project Center`, `Inbox`, and `Trash`. Assign Notion-style automated pipelines (hotwords, replacement rules, prompt presets, translation languages) per project.
- **📝 Rich Interactive Editor**: Synchronized playback editor supporting inline segment splitting/merging, speaker badges, version snapshots, formatting (bold, italic, underline, strikethrough `Ctrl+Shift+S`, inline code), and anticipation carets.
- **👥 Speaker Profiles & Review**: Build local speaker profiles, adjust speaker badges segment by segment, and batch-review suggested or anonymous speaker groups before export.
- **✨ LLM Assistant**: Polish, translate, and summarize transcripts using OpenAI, Anthropic, Gemini, DeepSeek, or Ollama.
- **🗣️ Live Caption & Voice Typing**: Reuse the offline live transcription stack for floating system captions or system-wide dictation into other applications.
- **📤 Smart Export**: Export in multiple formats (TXT, SRT, VTT, JSON, Markdown) with bilingual support and clipboard copy.
- **🛟 Recovery, Backup & Storage**: Resume interrupted tasks via the Task Center, export lightweight backups, manage storage directories, set custom FFmpeg paths, and configure audio retention cleanup policies.
- **🤖 Advanced AI Models**: Powered by state-of-the-art models including **Qwen3-ASR** (via llama.cpp), **FireRedASR2-AED**, **SenseVoice**, **Whisper**, **Paraformer**, and **Silero VAD v5**.

## 🚀 Getting Started

### Download from GitHub Releases

The easiest way to install Sona is to download the pre-built binaries for your platform from the [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) page.

### User Guide

For end-user setup and daily workflows, read the [User Guide](docs/user-guide.md). It covers first-run setup, `Live Record`, `Batch Import`, `Projects` / `Inbox`, transcript editing, speaker review, version snapshots, LLM features, `Voice Typing`, export, `Dashboard` / backup / recovery entry points, and troubleshooting.

## 📚 Documentation

Choose the guide that matches what you want to do:

| Resource | What it covers | Languages |
| --- | --- | --- |
| User Guide | Installation, first-run setup, transcription, editing, and export | [English](docs/user-guide.md) · [简体中文](docs/user-guide.zh-CN.md) |
| CLI Guide | Stateless local/online transcription, model management, and configuration | [English](docs/cli.md) · [简体中文](docs/cli.zh-CN.md) |
| HTTP API Reference | Server configuration, authentication, endpoints, and webhooks | [English](docs/api.md) · [简体中文](docs/api.zh-CN.md) |
| Development Guide | Local setup, testing, desktop builds, and CLI builds | [English](docs/development.md) · [简体中文](docs/development.zh-CN.md) |
| Architecture Guide | Package roles, dependency direction, compatibility boundaries, and verification | [English](docs/architecture.md) · [简体中文](docs/architecture.zh-CN.md) |
| Nightly Workflow | Nightly triggers, build jobs, artifacts, and publishing | [English](docs/nightly-workflow.md) · [简体中文](docs/nightly-workflow.zh-CN.md) |
| Contributing | Branches, validation, commits, and pull request expectations | [English](CONTRIBUTING.md) |

## 📄 License

This project is licensed under the [MIT License](LICENSE).
