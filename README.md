# Sona

Sona is a powerful, offline transcript editor built with Tauri, React, and Sherpa-onnx. It provides fast and accurate speech-to-text capabilities directly on your local machine, ensuring privacy and performance.

## Features

- **Offline Speech Recognition**: Powered by Sherpa-onnx and SenseVoice for high-accuracy transcription without an internet connection.
- **Real-time Transcription**: Record and transcribe audio in real-time.
- **Batch Import**: Import multiple audio/video files for bulk transcription.
- **Interactive Editor**: Edit transcripts with ease.
- **Waveform Visualization**: Beautiful audio visualization during playback and recording.
- **Privacy First**: All processing happens locally on your device.

## Tech Stack

- **Framework**: [Tauri v2](https://v2.tauri.app/)
- **Frontend**: [React](https://react.dev/), [TypeScript](https://www.typescriptlang.org/), [Vite](https://vitejs.dev/)
- **State Management**: [Zustand](https://github.com/pmndrs/zustand)
- **UI Components**: [Lucide React](https://lucide.dev/)
- **AI Engine**: [Sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)

## Prerequisites

- Node.js (v20 or later recommended)
- Rust (for Tauri backend)

## Getting Started

1.  **Clone the repository**

    ```bash
    git clone https://github.com/AirSodaz/sona.git
    cd sona
    ```

2.  **Install dependencies**

    ```bash
    npm install
    ```

3.  **Run in development mode**

    ```bash
    npm run tauri dev
    ```

    This command will start the frontend dev server and the Tauri application window.

## Building

To build the application for production:

```bash
npm run tauri build
```

The executable will be generated in `src-tauri/target/release/bundle`.
