/**
 * Application entry point.
 * Renders the root React component into the DOM.
 */

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LiveCaptionWindow } from "./windows/LiveCaptionWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import './i18n';
import { useTranscriptStore } from "./stores/transcriptStore";
import { useBatchQueueStore } from "./stores/batchQueueStore";
import { useDialogStore } from "./stores/dialogStore";
import { transcriptionService } from "./services/transcriptionService";
import { modelService } from "./services/modelService";

// Expose stores and services for E2E testing
if (import.meta.env.DEV) {
  (window as any).useTranscriptStore = useTranscriptStore;
  (window as any).useBatchQueueStore = useBatchQueueStore;
  (window as any).useDialogStore = useDialogStore;
  (window as any).transcriptionService = transcriptionService;
  (window as any).modelService = modelService;
}

let ComponentToRender = App;

// Check if we are in the live-caption window context
try {
  // getCurrentWindow() throws if not in Tauri context
  const currentWindow = getCurrentWindow();
  if (currentWindow.label === 'live-caption') {
    ComponentToRender = LiveCaptionWindow;
  }
} catch (e) {
  // Fallback to App if not running in Tauri or error
  // In browser dev mode, this is expected
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ComponentToRender />
  </React.StrictMode>,
);
