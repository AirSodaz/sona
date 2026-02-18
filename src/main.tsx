import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import './i18n';
import { useTranscriptStore } from "./stores/transcriptStore";
import { useBatchQueueStore } from "./stores/batchQueueStore";
import { useDialogStore } from "./stores/dialogStore";
import { transcriptionService } from "./services/transcriptionService";
import { modelService } from "./services/modelService";
import { CaptionWindow } from "./components/CaptionWindow";

// Expose stores and services for E2E testing
if (import.meta.env.DEV) {
  (window as any).useTranscriptStore = useTranscriptStore;
  (window as any).useBatchQueueStore = useBatchQueueStore;
  (window as any).useDialogStore = useDialogStore;
  (window as any).transcriptionService = transcriptionService;
  (window as any).modelService = modelService;
}

const isCaptionWindow = window.location.search.includes('window=caption');

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isCaptionWindow ? <CaptionWindow /> : <App />}
  </React.StrictMode>,
);
