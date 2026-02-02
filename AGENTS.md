# AGENTS.md

## 1. Project Overview
**Sona** is a privacy-first, offline transcript editor built with **Tauri**, **React**, and **Sherpa-onnx**. It performs real-time and batch speech-to-text processing locally on the user's machine using a unique architecture where a Rust backend orchestrates a Node.js sidecar for AI model execution.

### Key Features
*   **Offline/Private**: No data leaves the device.
*   **Sidecar Architecture**: AI inference runs in a detached Node.js process managed by Tauri.
*   **Real-time & Batch**: Supports both live recording and file import.
*   **Performance**: Optimized audio processing loop and virtualized lists for long transcripts.

## 2. Tech Stack

### Frontend
*   **Framework**: React 19 + TypeScript
*   **Build Tool**: Vite
*   **State Management**: Zustand
*   **Styling**: Pure CSS with Variables (No Tailwind). Design system defined in `src/styles/index.css`.
*   **Icons**: Lucide React
*   **Internationalization**: i18next (English/Chinese)

### Backend (Tauri)
*   **Core**: Tauri v2 (Rust)
*   **Capabilities**: `shell` (for sidecar), `fs`, `dialog`, `http`.
*   **Configuration**: `src-tauri/tauri.conf.json`.

### AI Engine (Sidecar)
*   **Runtime**: Node.js (packaged binary).
*   **Inference**: `sherpa-onnx-node`.
*   **Communication**: Stdin/Stdout streams between Tauri and Sidecar.
*   **Location**: `src-tauri/sidecar/`.

## 3. Architecture & Critical Components

### 3.1. Sidecar Architecture
The application relies on a "Sidecar" pattern. The frontend does NOT run the AI models directly.
1.  **Tauri** spawns a Node.js binary (bundled in `src-tauri/binaries/`).
2.  **Script**: The binary executes `src-tauri/sidecar/dist/index.mjs` (compiled from `sherpa-recognizer.js`).
3.  **Communication**:
    *   **Audio Input**: Frontend sends raw PCM audio (Int16) via `transcriptionService.ts` -> Tauri Command -> Sidecar Stdin.
    *   **Transcript Output**: Sidecar prints JSON lines to Stdout -> Captured by Tauri -> Parsed by `transcriptionService.ts`.

### 3.2. Critical Services

#### `src/services/transcriptionService.ts`
This is the bridge between the React frontend and the AI Sidecar.
*   **Lifecycle**: Manages `spawn` and `kill` of the sidecar process.
*   **Audio Streaming**:
    *   Converts `Float32Array` (Web Audio API) to `Int16Array`.
    *   **Performance Optimization**: Uses manual `if/else` clamping loops instead of `Math.max/min` to minimize overhead in high-frequency audio callbacks.
*   **Output Handling**:
    *   Uses `StreamLineBuffer` (`src/utils/streamBuffer.ts`) to reconstruct fragmented stdout chunks into valid JSON lines.
    *   Parses JSON to `TranscriptSegment` objects.

#### `src/stores/transcriptStore.ts`
Zustand store for managing application state.
*   **Data**: Holds `segments` (array of transcript lines), `isRecording`, `isLoading`, etc.
*   **Optimization**:
    *   `upsertSegment`: Optimized for streaming. Checks the last segment for updates to avoid O(N) searches.
    *   `findSegmentAndIndexForTime`: Uses index hinting for O(1) lookups during sequential playback.

#### `src-tauri/sidecar/sherpa-recognizer.js`
The entry point for the AI engine.
*   **Modes**:
    *   `stream`: Real-time recognition. Reads PCM bytes from stdin.
    *   `batch`: File transcription.
*   **Output**: Emits JSON lines. `{ text: "...", start: 0.5, end: 1.2, isFinal: true }`.

## 4. Coding Standards & Guidelines

**All code and documentation must follow [Google Style Guides](https://google.github.io/styleguide/).**

### 4.1. General Rules
1.  **Google Style**:
    *   **TypeScript/JS**: Follow [Google JavaScript Style Guide](https://google.github.io/styleguide/jsguide.html). Use `const` over `let`, 2-space indentation, clear variable naming.
    *   **Docstrings**: All exported functions and classes must have JSDoc comments (`/** ... */`).
2.  **Strict Typing**: No `any`. Define interfaces in `src/types/`.
3.  **No Tailwind**: Do NOT introduce Tailwind CSS. Use standard CSS classes and variables defined in `src/styles/index.css`.

### 4.2. React Best Practices
*   **Functional Components**: Use functional components with Hooks.
*   **Performance**: Use `useMemo` and `useCallback` for expensive operations or stable references passed to children.
*   **Virtualization**: Use `react-virtuoso` for long lists (e.g., transcripts) to maintain UI frame rate.

### 4.3. Accessibility (A11y)
*   **Interactive Elements**: All buttons/inputs must have `aria-label` or visible labels.
*   **Custom Controls**: Custom UI (like specific toggle switches) must use appropriate ARIA roles (`role="switch"`, `aria-checked`, etc.).
*   **Focus Management**: Ensure `:focus-visible` styles are present for keyboard navigation.

## 5. Directory Structure

```
├── src/
│   ├── components/       # React components
│   ├── hooks/            # Custom React hooks
│   ├── locales/          # i18n JSON files
│   ├── services/         # Singleton services (transcription, model management)
│   ├── stores/           # Zustand stores
│   ├── styles/           # Global CSS and Design System
│   ├── types/            # TypeScript interfaces
│   ├── utils/            # Helper functions
│   ├── App.tsx           # Main component
│   └── main.tsx          # Entry point
├── src-tauri/
│   ├── sidecar/          # Node.js AI Engine source
│   ├── binaries/         # Compiled Node.js binaries (platform specific)
│   ├── src/              # Rust backend source
│   └── tauri.conf.json   # Application configuration
└── package.json
```

## 6. Testing Strategy

*   **Framework**: Vitest for unit/integration tests.
*   **Mocks**:
    *   Mock `transcriptionService` for component tests involving recording.
    *   Mock Tauri APIs (`@tauri-apps/plugin-shell`, etc.) when testing services that interact with the backend.
*   **Performance**: Use `.perf.test.ts` for benchmarking critical paths (e.g., audio loop, segment lookup).

## 7. Development Workflow

1.  **Install Dependencies**: `npm install` (Runs `setup-sidecar` automatically).
2.  **Dev Server**: `npm run tauri dev`.
3.  **Build**: `npm run tauri build`.

**Important**: If you modify `src-tauri/sidecar/sherpa-recognizer.js`, you must verify the build pipeline correctly bundles it into `src-tauri/sidecar/dist/`.

## 8. UI/UX Design Guidelines

The application follows a **Warm Minimalism** design philosophy, inspired by tools like Notion and modernized with softer, organic touches. The goal is to create a workspace that feels calm, focused, and human, rather than sterile or purely utility-driven.

### 8.1. Core Principles
*   **Warmth over Sterility**: Avoid pure `#000000` black or `#FFFFFF` white. Use off-whites (`#FBFBFA`), warm grays, and soft charcoal for text.
*   **Content First**: The UI should recede. Generous whitespace and a lack of heavy borders allow the content (transcripts) to breathe.
*   **Tactile Feedback**: Interactive elements should have subtle states (hover, active) that feel responsive but not jumpy. Use soft shadows (`--shadow-sm`) instead of harsh outlines.
*   **Typography**:
    *   **Interface**: `Inter` for clean, legible UI text.
    *   **Content**: `Merriweather` (or similar serif) for long-form reading (transcripts), evoking a book-like quality.
    *   **Data/Code**: `JetBrains Mono` for timestamps and technical data.

### 8.2. Design Tokens (ref: `src/styles/index.css`)
*   **Backgrounds**: Layered warm neutrals.
    *   Base: `--color-bg-primary` (Paper-like)
    *   Sidebar/Header: `--color-bg-secondary` (Slightly darker warm gray)
*   **Accents**: Use "Earthy" tones for actions rather than neon or primary colors.
    *   Success: Muted Green (`#4ea067`)
    *   Warning: Muted Yellow/Orange
    *   Error: Soft Red (`#e03e3e`)
*   **Radius**: Use softer border radii (`6px` to `12px`) to avoid sharp, aggressive corners.
