# Sona

**Sona** is a powerful, offline transcript editor built with [Tauri](https://tauri.app), [React](https://react.dev), and [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). It provides fast, accurate, and private speech-to-text capabilities directly on your local machine.

## ✨ Features

- **🔒 Offline & Private**: All speech processing happens locally on your device. No data leaves your machine.
- **🎙️ Real-time Transcription**: Record and transcribe audio in real-time with low latency.
- **📁 Batch Processing**: Import multiple audio or video files for bulk transcription in the background.
- **📝 Interactive Editor**: A rich text editor synchronized with audio playback for easy corrections.
- **🌊 Waveform Visualization**: Beautiful, real-time audio visualization during recording and playback.
- **🤖 Advanced AI Models**: Powered by state-of-the-art models like **SenseVoice**, **Whisper**, and **Paraformer**.

## 🚀 Getting Started

### Prerequisites

*   **Node.js**: v20 or later.
*   **Rust**: Stable release (required for the Tauri backend).
*   **Package Manager**: `npm` (recommended).

#### Linux Requirements
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
    librsvg2-dev
```

### Installation

1.  **Clone the repository**
    ```bash
    git clone https://github.com/AirSodaz/sona.git
    cd sona
    ```

2.  **Install dependencies**
    ```bash
    npm install
    ```
    *Note: This automatically triggers the `setup-sidecar` script to configure the transcription engine.*

3.  **Run the application**
    ```bash
    npm run tauri dev
    ```

## 📦 Model Management

Sona allows you to choose the AI model that best fits your needs.

1.  **Launch Sona**.
2.  Navigate to **Settings > Models**.
3.  Choose from a curated list of high-performance models:
    *   **SenseVoice**: Best for multilingual support and emotion recognition.
    *   **Whisper (Tiny)**: Lightweight version of OpenAI's Whisper model.
    *   **Paraformer**: Optimized for streaming.
4.  Click **Download**. The model will be automatically stored locally.

## 🤝 Contributing

We welcome contributions! For developers, please refer to **`AGENTS.md`** for detailed architectural documentation, coding standards, and testing guidelines.

## 🏗️ Building

To build the application for production:

```bash
npm run tauri build
```

The executable will be generated in `src-tauri/target/release/bundle`.
