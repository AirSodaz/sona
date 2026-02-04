# AGENTS.md

## 1. Project Overview
**Sona** is a privacy-first, offline transcript editor built with **Tauri**, **React**, and **Sherpa-onnx**. It performs real-time and batch speech-to-text processing locally on the user's machine.

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
*   **Icons**: Lucide React (Centralized in `src/components/Icons.tsx`)
*   **Internationalization**: i18next (English/Chinese). *Note: New strings must be added to both `src/locales/en.json` and `zh.json`.*
*   **Virtualization**: `react-virtuoso` for transcript lists.

### Backend (Tauri)
*   **Core**: Tauri v2 (Rust)
*   **Capabilities**: `shell` (for sidecar), `fs`, `dialog`, `http`.
*   **Configuration**: `src-tauri/tauri.conf.json`.

### AI Engine (Sidecar)
*   **Runtime**: Node.js (packaged binary).
*   **Inference**: `sherpa-onnx-node`.
*   **Communication**: Stdin/Stdout streams between Tauri and Sidecar.
*   **Location**: `src-tauri/sidecar/` (Entry: `sherpa-recognizer.js`).

## 3. Architecture & Critical Components

### 3.1. Sidecar Architecture
The frontend does NOT run AI models directly.
1.  **Tauri** spawns a Node.js binary (bundled in `src-tauri/binaries/`).
2.  **Script**: Executes `src-tauri/sidecar/dist/index.mjs` (compiled from `sherpa-recognizer.js`).
3.  **Communication**:
    *   **Audio Input**: Frontend sends `Int16Array` (converted from Float32 via `public/audio-processor.js` AudioWorklet) -> Tauri Command -> Sidecar Stdin.
    *   **Output**: Sidecar prints JSON lines to Stdout -> Captured by Tauri -> Parsed by `transcriptionService.ts`.

### 3.2. Critical Services & Stores
*   **`transcriptionService.ts`**: Manages sidecar lifecycle (`spawn`/`kill`). Uses `StreamLineBuffer` to reconstruct JSON from stdout chunks.
*   **`transcriptStore.ts`**:
    *   **`upsertSegment`**: Optimized for streaming; checks last segment first.
    *   **`findSegmentAndIndexForTime`**: Uses index hinting for O(1) lookups during playback.
    *   **`setActiveSegmentId`**: Accepts an optional `index` to allow O(1) updates.
*   **`segmentUtils.ts`**: Uses `getEffectiveLength` (regex-based loop) instead of string replacement for 28% faster length calculations.

### 3.3. Performance Optimizations
*   **High-Frequency Updates**: Components like `AudioPlayer` (TimeDisplay, SeekSlider) use `useRef` and direct `store.subscribe` to bypass React render cycles for `currentTime`.
*   **Granular Selectors**: `App.tsx` selects specific state slices (e.g., `state.config.theme`) to avoid full-tree re-renders.

## 4. Coding Standards & Guidelines

**All code and documentation must follow [Google Style Guides](https://google.github.io/styleguide/).**

### 4.1. General Rules
*   **Style**: 2-space indentation, clear variable naming.
*   **Docstrings**: All exported functions/classes must have JSDoc comments.
*   **Strict Typing**: No `any`. Define interfaces in `src/types/`.
*   **No Tailwind**: Use standard CSS classes and variables (`src/styles/index.css`).
*   **Named Functions**: Use `function ComponentName() { ... }` instead of arrow functions for components.

### 4.2. React Best Practices
*   **Functional Components**: Use hooks.
*   **Memoization**: `useMemo`/`useCallback` for stable references.
*   **Complex Logic**: Refactor nested ternary operators into helper functions or `switch` statements.
*   **Audio/Timer Logic**:
    *   `RecordingTimer` uses `useRef` for `isPaused` to prevent `setInterval` closures from capturing stale state.
    *   `LiveRecord` delegates timer logic to `RecordingTimer` to isolate updates.

### 4.3. Accessibility (A11y)
*   **Interactive Elements**: All buttons/inputs must have `aria-label` or visible labels.
*   **Nested Controls**: "Buttons" inside interactive containers (like `BatchImport` drop zone) must be `div`s with `aria-hidden="true"` and `pointer-events: none` to prevent invalid HTML.
*   **Disabled State**: Use `data-tooltip` to explain why a button is disabled.
*   **Modals**:
    *   Implement focus traps (listen for `Tab`/`Shift+Tab`).
    *   Handle `Escape` key via global listener checking `useDialogStore.getState().isOpen`.
    *   Manage focus restoration on unmount.
*   **Lists**: Use `role="list"`/`role="listitem"` for complex lists (like FileQueue) instead of `listbox` if items contain nested actions.

## 5. Testing Strategy

### 5.1. Unit Testing (Vitest)
*   **Mocking**:
    *   Mock module paths relative to the **test file** (e.g., `vi.mock('../../hooks/...')`), not the source file.
    *   `vi.mocked()` values persist; explicitly reset conflicting mocks (e.g., `.mockResolvedValue`) inside tests or `beforeEach`.
    *   `transcriptionService` tests using `@tauri-apps/plugin-shell` must mock the `Child` process and emit 'close' events manually.
*   **Environment**: `src-tauri/src/hardware.rs` tests run via `cargo test`.

### 5.2. E2E Testing (Playwright)
*   **Initialization**:
    *   Inject `window.__TAURI_INTERNALS__` via `page.add_init_script` (mocks must be ready before app load).
    *   Inject `sona-config` into `localStorage` to bypass "Model Setup" overlays.
*   **Mocking Tauri**:
    *   Mock `plugin:fs|exists` and `plugin:path|resolve_directory` to prevent crashes.
    *   Mock `plugin:dialog|open` to return flat arrays of file paths.
*   **Quirks**:
    *   Use `page.keyboard.press('Escape')` to close modals (avoids pointer interception).
    *   Use `exact: true` for text locators (e.g., distinguishing "Language" label from selected values).
    *   `AudioPlayer` is conditional (`null` if no `audioUrl`); inject a valid URL into the store to test it.
    *   `BatchImport` verification needs `plugin:fs` mocks.

## 6. Directory Structure & Constraints

*   `src-tauri/sidecar/`: Node.js AI Engine.
    *   **Do NOT edit `dist/` directly**. Edit `sherpa-recognizer.js` and verify the build.
*   `src/locales/`: i18n JSONs. Update both `en.json` and `zh.json`.
*   `.Jules/palette.md`: Contains UX/accessibility learnings. Append new findings here.

## 7. Troubleshooting & Common Issues
*   **Timeouts in `Settings.test.tsx`**: Often due to async `useEffect`. Use `waitFor` instead of `act`.
*   **ESM Modules**: Playwright tests cannot use `__dirname`; use `fileURLToPath(import.meta.url)`.
*   **Act Warnings**: Resolve by awaiting state changes triggered by effects.
