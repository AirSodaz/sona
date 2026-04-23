import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import './i18n';
import { useTranscriptStore } from "./stores/transcriptStore";
import { useBatchQueueStore } from "./stores/batchQueueStore";
import { useDialogStore } from "./stores/dialogStore";
import { useOnboardingStore } from "./stores/onboardingStore";
import { transcriptionService } from "./services/transcriptionService";
import { modelService } from "./services/modelService";
import { voiceTypingService } from "./services/voiceTypingService";
import { CaptionWindow } from "./components/CaptionWindow";
import { VoiceTypingOverlay } from "./components/VoiceTypingOverlay";

// Expose stores and services for E2E testing
if (import.meta.env.DEV) {
  (window as any).useTranscriptStore = useTranscriptStore;
  (window as any).useBatchQueueStore = useBatchQueueStore;
  (window as any).useDialogStore = useDialogStore;
  (window as any).useOnboardingStore = useOnboardingStore;
  (window as any).transcriptionService = transcriptionService;
  (window as any).modelService = modelService;
  (window as any).voiceTypingService = voiceTypingService;
}

const isCaptionWindow = window.location.search.includes('window=caption');
const isVoiceTypingWindow = window.location.search.includes('window=voice-typing');

let rootComponent = <App />;
if (isVoiceTypingWindow) {
  rootComponent = <VoiceTypingOverlay />;
} else if (isCaptionWindow) {
  rootComponent = <CaptionWindow />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {rootComponent}
  </React.StrictMode>,
);
