# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

Sona is a privacy-first desktop transcript editor built with Tauri v2, React 19, TypeScript, Zustand, and a Rust backend powered by sherpa-onnx. The main product flow is offline transcription; cloud AI is optional and only used for transcript polish/translation.

Key product modes:
- Live recording with real-time transcription
- Batch import with background queue processing
- Transcript editing with synchronized playback
- Optional AI polish/translate via OpenAI, Anthropic, Gemini, or Ollama
- Local history, export, and model management

## Common commands

### Frontend and desktop app
- `npm run dev` — start the Vite dev server on port 1420
- `npm run tauri dev` — run the full desktop app in Tauri dev mode
- `npm run build` — TypeScript check + production frontend build
- `npm run tauri build` — build the production desktop bundle
- `npm run preview` — preview the production frontend build

### Tests
- `npm test` — run the Vitest suite
- `npx vitest src/path/to/file.test.tsx` — run one Vitest file
- `npx vitest -t "test name"` — run Vitest tests matching a name
- `npx playwright test` — run Playwright end-to-end tests
- `npx playwright test tests/e2e/example.spec.ts` — run one Playwright file
- `npx playwright test -g "test title"` — run one Playwright test by title
- `cargo test --manifest-path src-tauri/Cargo.toml` — run Rust tests
- `cargo test --manifest-path src-tauri/Cargo.toml test_name` — run a specific Rust test

### CLI and packaging helpers
- `cargo run --manifest-path src-tauri/Cargo.toml --bin sona-cli -- transcribe ./sample.mp4 --config ./sona-cli.toml --output ./sample.srt` — run the offline CLI from source
- `npm run verify:cli-bundle` — verify the packaged CLI bundle

### Notes
- There is currently no verified `lint` script in the root `package.json`.
- `npm run build` triggers `scripts/setup-ffmpeg.js` via `prebuild`.
- Tauri build/dev flows go through `scripts/tauri.js`.

## High-level architecture

### Frontend shell
- `src/main.tsx` boots either the main app or the caption-only window depending on query params.
- `src/App.tsx` is the main orchestrator for app mode, initialization hooks, dialog rendering, and top-level layout.

### State model
- `src/stores/transcriptStore.ts` is the central source of truth for transcript segments, playback/recording state, current mode, config, and AI task state.
- `src/stores/batchQueueStore.ts` manages the batch queue, concurrency, progress, auto-polish integration, and handoff into history.
- `src/stores/historyStore.ts` manages the persisted transcript/audio history index.
- `src/stores/onboardingStore.ts` tracks first-run setup and onboarding UI state.

### Live transcription flow
1. The frontend records audio through hooks in `src/hooks/useAudioRecorder.ts`.
2. `src/services/transcriptionService.ts` manages recognizer lifecycle and frontend/backend synchronization.
3. Rust commands in `src-tauri/src/lib.rs` dispatch into `src-tauri/src/sherpa.rs` and `src-tauri/src/audio.rs`.
4. The Rust backend emits recognizer output events back to the frontend, where transcript segments are upserted into the store.

### Batch transcription flow
- Batch jobs are coordinated from `src/stores/batchQueueStore.ts`.
- Rust batch processing lives primarily in `src-tauri/src/sherpa.rs` and `src-tauri/src/pipeline.rs`.
- ffmpeg is used to decode/resample input before offline inference.
- Completed batch items are written into history and can become the active transcript context.

### AI assistant flow
- `src/services/polishService.ts` and `src/services/translationService.ts` chunk transcript segments, build prompts, call the backend, parse strict JSON responses, and progressively update transcript/history content.
- `src-tauri/src/ai.rs` is the provider adapter/proxy for OpenAI-compatible APIs, Anthropic, Gemini, and Ollama.
- If the user navigates away during AI processing, services may update the backing history transcript directly rather than only mutating the active in-memory state.

### Persistence and setup
- Main user config is persisted in localStorage under `sona-config`.
- `src/hooks/useAppInitialization.ts` loads config, applies theme/font settings, and syncs tray-close behavior to the Rust backend.
- `src/hooks/useAutoSaveTranscript.ts` debounces transcript persistence and flushes on context switches to avoid overwriting another history item.
- `src/services/modelService.ts` coordinates model download/extract flows with Tauri commands.
- `src/services/historyService.ts` owns transcript/audio persistence under the app-local history directory.

## Key implementation constraints

### UI and styling
- Use pure CSS and existing variables in `src/styles/index.css`. Do not add Tailwind.
- Use Lucide icons through the shared icon layer in `src/components/Icons.tsx`.
- Prefer named function components instead of arrow-function components.

### Typing and docs
- Follow Google style guidance already used in this repo.
- Use 2-space indentation.
- Do not introduce `any`.
- Add JSDoc for exported functions and classes.
- Put shared interfaces/types in `src/types/` when they need to be reused.

### React and performance
- Prefer granular Zustand selectors to avoid whole-tree rerenders.
- For high-frequency audio/playback state, follow the existing `useRef` + direct subscription patterns instead of pushing everything through React render cycles.
- Avoid nested ternaries in complex rendering logic; extract helpers or use `switch`.

### i18n
- This app uses `react-i18next`.
- Any new user-facing string must be added to both:
  - `src/locales/en.json`
  - `src/locales/zh.json`

### Accessibility
- Interactive controls must have an `aria-label` or a visible label.
- In interactive containers, do not nest actual buttons inside other interactive elements; follow the existing `aria-hidden` + `pointer-events: none` pattern where needed.
- Disabled controls should explain why via `data-tooltip` when appropriate.
- Modal work must preserve focus trap, Escape handling, and focus restoration patterns.
- For complex lists with nested actions, prefer `role="list"` / `role="listitem"` over `listbox`.

## Testing guidance

### Vitest
- Vitest runs in jsdom and excludes `tests/e2e/**`.
- When mocking modules, mock paths relative to the test file, not the source file.
- `vi.mocked()` state can persist between tests; reset conflicting mock behavior in `beforeEach` or inside the test.

### Playwright
- Inject Tauri mocks before app load with `page.add_init_script`.
- Seed `sona-config` in localStorage when tests need to bypass first-run/model-setup UI.
- Mock `plugin:fs|exists`, `plugin:path|resolve_directory`, and dialog APIs when flows depend on them.
- Prefer `page.keyboard.press('Escape')` to close modals when pointer interactions are flaky.
- Use `exact: true` for ambiguous text locators.
- `AudioPlayer` only renders when an `audioUrl` exists.

### Rust tests
- Rust-side tests run with `cargo test --manifest-path src-tauri/Cargo.toml`.
- CLI integration coverage lives under `src-tauri/tests/`.

## Directory cues

- `src/components/settings/` — settings tab implementations
- `src/services/` — frontend orchestration for transcription, AI, history, export, and models
- `src/stores/` — Zustand stores; start here when behavior spans multiple screens
- `src-tauri/src/` — Tauri command handlers, audio capture, AI proxy, Sherpa integration, pipeline code
- `docs/user-guide.md` — end-user workflows and setup details
- `docs/cli.md` — CLI usage and configuration
