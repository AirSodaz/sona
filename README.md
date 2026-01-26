# Sona

**Sona** is a powerful, offline transcript editor built with [Tauri](https://tauri.app), [React](https://react.dev), and [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). It provides fast, accurate, and private speech-to-text capabilities directly on your local machine.

## ✨ Features

- **🔒 Offline & Private**: All speech processing happens locally on your device. No data leaves your machine.
- **🎙️ Real-time Transcription**: Record and transcribe audio in real-time with low latency.
- **📁 Batch Processing**: Import multiple audio or video files for bulk transcription in the background.
- **📝 Interactive Editor**: A rich text editor synchronized with audio playback for easy corrections.
- **🌊 Waveform Visualization**: Beautiful, real-time audio visualization during recording and playback.
- **🤖 Advanced AI Models**: Powered by state-of-the-art models like **SenseVoice**, **Whisper**, and **Paraformer**.

## 🛠️ Tech Stack

- **Core**: [Tauri v2](https://v2.tauri.app/) (Rust)
- **Frontend**: [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/), [Vite](https://vitejs.dev/)
- **State Management**: [Zustand](https://github.com/pmndrs/zustand)
- **UI Components**: [Lucide React](https://lucide.dev/)
- **AI Engine**: [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (running in a Node.js sidecar)

## 📦 Model Management

Sona does not bundle AI models by default to keep the installer size manageable. Instead, it features a built-in **Model Manager**.

1.  **Launch Sona**.
2.  Navigate to **Settings > Models**.
3.  Choose from a curated list of high-performance models:
    *   **SenseVoice**: Best for multilingual support (Chinese, English, Japanese, Korean, Cantonese) and emotion recognition.
    *   **Whisper (Tiny)**: Lightweight version of OpenAI's Whisper model (Int8 quantized).
    *   **Paraformer/Zipformer**: Specialized models optimized for streaming and specific language pairs.
4.  Click **Download**. The model will be automatically downloaded and stored in your system's application data directory.

## 🚀 Getting Started

### Prerequisites

*   **Node.js**: v20 or later.
*   **Rust**: Stable release (required for the Tauri backend).
*   **Package Manager**: `npm` (recommended).

#### Linux Requirements
If you are developing on Linux (Ubuntu/Debian), ensure you have the necessary system dependencies:

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
    *Note: This automatically triggers the `setup-sidecar` script, which downloads the necessary Node.js binary for the transcription engine.*

3.  **Run in development mode**
    ```bash
    npm run tauri dev
    ```
    This command starts the frontend dev server and the Tauri application window.

## 🤝 Contributing

We welcome contributions! Sona uses a unique architecture where the heavy lifting of speech recognition is done by a **Node.js Sidecar** (`src-tauri/sidecar`) managed by the Rust backend.

### Project Structure
*   `src/`: **Frontend**. The React user interface.
*   `src-tauri/`: **Backend**. Rust code that handles window management, file system access, and sidecar orchestration.
*   `src-tauri/sidecar/`: **AI Engine**. A standalone Node.js application (`sherpa-recognizer.js`) that interfaces with `sherpa-onnx-node`.

### Development Notes
*   **Sidecar Setup**: The sidecar requires a standalone Node.js binary to run independently of the user's system Node.js. This is handled by `scripts/setup-sidecar.js`. If you encounter sidecar issues, try running `npm run setup-sidecar`.
*   **Audio Testing**: Testing audio features usually requires a real microphone. The `scripts/setup-sidecar.js` script also configures the environment for the sidecar build.

## 🏗️ Building

To build the application for production:

```bash
npm run tauri build
```

The executable will be generated in `src-tauri/target/release/bundle`.
