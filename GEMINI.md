# Sona - Gemini CLI Context

This file provides essential context for Gemini CLI when working within the Sona codebase.

## Project Overview

Sona is a privacy-first, offline transcript editor built with **Tauri v2**, **React 19**, and **Rust**. It utilizes **Sherpa-onnx** for high-performance, local speech-to-text processing.

### Key Features
- **Offline Transcription:** Real-time recording and batch processing using local models (SenseVoice, Whisper, Paraformer).
- **Interactive Editor:** Rich text editing synchronized with audio playback.
- **LLM Assistant:** Optional cloud-based transcript polishing and translation (OpenAI, Anthropic, Gemini, Ollama).
- **Multi-Format Export:** Supports TXT, SRT, VTT, and JSON with bilingual options.

### Tech Stack
- **Frontend:** React 19, TypeScript, Zustand (State Management), Vite (Build Tool).
- **Backend:** Rust, Tauri v2.
- **Styling:** Pure CSS (No Tailwind).
- **Icons:** Lucide React.
- **i18n:** react-i18next (supports English and Simplified Chinese).

---

## Building and Running

### Development
- `npm run tauri dev` — Start the full desktop application in development mode.
- `npm run dev` — Run the Vite development server only (port 1420).

### Testing
- `npm test` — Run the Vitest unit/component test suite.
- `npx vitest src/path/to/file.test.tsx` — Run a specific Vitest file.
- `npx playwright test` — Run Playwright end-to-end tests.
- `cargo test --manifest-path src-tauri/Cargo.toml` — Run Rust backend tests.

### Production
- `npm run tauri build` — Build the production desktop application bundle.
- `npm run build` — TypeScript check and production frontend build.

### CLI
- `cargo run --manifest-path src-tauri/Cargo.toml -- transcribe <input> --config <toml> --output <output>` — Run the offline CLI from source.

---

## Project Structure

- `src/` — React frontend source code.
  - `components/` — UI components (Layout, Settings, Editor, etc.).
  - `hooks/` — React hooks for logic and lifecycle.
  - `services/` — Orchestration for transcription, LLM, history, and models.
  - `stores/` — Zustand store definitions.
  - `locales/` — i18n translation files (EN/ZH).
- `src-tauri/` — Rust backend and Tauri configuration.
  - `src/` — Command handlers, Sherpa integration, and LLM proxy.
  - `tauri.conf.json` — Tauri application configuration.
- `docs/` — User and developer documentation.
- `scripts/` — Build and setup helper scripts.

---

## Development Conventions

### Coding Style
- **Indentation:** 2 spaces.
- **React:** Prefer named function components over arrow functions.
- **TypeScript:** Strict typing; do not use `any`. Follow Google style guidance.
- **Styling:** Use pure CSS and existing variables in `src/styles/index.css`. **Do not add Tailwind CSS.**
- **Icons:** Use the shared icon layer in `src/components/Icons.tsx`.

### Internationalization (i18n)
- All user-facing strings must be added to both `src/locales/en.json` and `src/locales/zh.json`.

### State Management
- Use granular Zustand selectors to minimize re-renders.
- High-frequency state (e.g., audio playback) should use `useRef` + direct subscriptions where possible.

### Performance & Security
- All speech processing should remain offline by default.
- Cloud LLM features are optional and must be explicitly configured by the user.
- Interactive controls must have appropriate ARIA labels for accessibility.
