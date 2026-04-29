# Sona - Gemini CLI Context

This file provides essential context for Gemini CLI when working within the Sona codebase.

## Project Overview

Sona is a privacy-first, offline transcript editor built with **Tauri v2**, **React 19**, and **Rust**. It utilizes **Sherpa-onnx** for high-performance, local speech-to-text processing.

### Key Features
- **Unified Workbench:** A consolidated transcript editor (`TranscriptWorkbench`) that integrates audio playback, segment editing, and AI tools.
- **Offline Transcription:** Real-time recording and batch processing using local models (SenseVoice, Whisper, Paraformer).
- **AI-Powered Utilities:** Optional cloud-based transcript polishing, translation, and summaries via a modal-driven interface.
- **Diagnostics & Recovery:** Robust system health checks and a recovery center for interrupted tasks.
- **Automation & Pipelines:** Customizable rules for post-processing and transcript management.
- **Multi-Format Export:** Supports TXT, SRT, VTT, and JSON with bilingual options.

### Tech Stack
- **Frontend:** React 19, TypeScript, Zustand (State Management), Vite.
- **Backend:** Rust, Tauri v2.
- **Styling:** Pure CSS (No Tailwind).
- **Icons:** Custom SVG wrappers in `src/components/Icons.tsx`.
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

---

## Project Structure

- `src/` — React frontend source code.
  - `components/` — UI components. Key component: `TranscriptWorkbench.tsx` (the core editor view).
  - `hooks/` — React hooks for logic and lifecycle.
  - `services/` — Orchestration for transcription, LLM, history, and models.
  - `stores/` — Zustand store definitions (`transcriptStore`, `historyStore`, `dialogStore`, etc.).
  - `locales/` — i18n translation files (EN/ZH).
- `src-tauri/` — Rust backend and Tauri configuration.
- `scripts/` — Build and setup helper scripts.

---

## Development Conventions

### Coding Style
- **Indentation:** 2 spaces.
- **React:** Prefer named function components over arrow functions.
- **TypeScript:** Strict typing; do not use `any`.
- **Styling:** Use pure CSS and existing variables in `src/App.css` and `src/styles/`. **Do not add Tailwind CSS.**
- **Icons:** Use the custom icons in `src/components/Icons.tsx`. Size them using `width` and `height` props (e.g., `<SettingsIcon width={20} height={20} />`).

### Global Dialogs
- Use `useDialogStore` for standard UI interactions:
  - `alert(message, options)`: Simple notification.
  - `confirm(message, options)`: Returns `Promise<boolean>`.
  - `prompt(message, options)`: Returns `Promise<string | null>`, useful for text inputs like renaming.
  - `showError(errorInput)`: Standardized error reporting.

### State Management
- Use granular Zustand selectors to minimize re-renders.
- High-frequency state (e.g., audio playback) should use `useRef` + direct subscriptions where possible.

### Internationalization (i18n)
- All user-facing strings must be added to both `src/locales/en.json` and `src/locales/zh.json`.
- Access translations via the `useTranslation` hook or the global `i18n` instance for services.

### Performance & Security
- All speech processing should remain offline by default.
- Cloud LLM features (Summary, Polish, Translate) are optional and require user configuration.
- Interactive controls must have appropriate ARIA labels for accessibility.
