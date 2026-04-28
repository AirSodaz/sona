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

declare global {
  interface Window {
    useTranscriptStore?: typeof useTranscriptStore;
    useBatchQueueStore?: typeof useBatchQueueStore;
    useDialogStore?: typeof useDialogStore;
    useOnboardingStore?: typeof useOnboardingStore;
    transcriptionService?: typeof transcriptionService;
    modelService?: typeof modelService;
    voiceTypingService?: typeof voiceTypingService;
  }
}

// Expose stores and services for E2E testing
if (import.meta.env.DEV) {
  window.useTranscriptStore = useTranscriptStore;
  window.useBatchQueueStore = useBatchQueueStore;
  window.useDialogStore = useDialogStore;
  window.useOnboardingStore = useOnboardingStore;
  window.transcriptionService = transcriptionService;
  window.modelService = modelService;
  window.voiceTypingService = voiceTypingService;
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
