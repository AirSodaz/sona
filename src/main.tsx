import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import './i18n';
import { useBatchQueueStore } from "./stores/batchQueueStore";
import { useDialogStore } from "./stores/dialogStore";
import { useOnboardingStore } from "./stores/onboardingStore";
import { useTranscriptPlaybackStore } from "./stores/transcriptPlaybackStore";
import { useTranscriptRuntimeStore } from "./stores/transcriptRuntimeStore";
import { useTranscriptSessionStore } from "./stores/transcriptSessionStore";
import { useTranscriptSidecarStore } from "./stores/transcriptSidecarStore";
import { transcriptionService } from "./services/transcriptionService";
import { modelService } from "./services/modelService";
import { voiceTypingService } from "./services/voiceTypingService";
import { CaptionWindow } from "./components/CaptionWindow";
import { VoiceTypingOverlay } from "./components/VoiceTypingOverlay";

declare global {
  interface Window {
    useTranscriptSessionStore?: typeof useTranscriptSessionStore;
    useTranscriptPlaybackStore?: typeof useTranscriptPlaybackStore;
    useTranscriptRuntimeStore?: typeof useTranscriptRuntimeStore;
    useTranscriptSidecarStore?: typeof useTranscriptSidecarStore;
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
  window.useTranscriptSessionStore = useTranscriptSessionStore;
  window.useTranscriptPlaybackStore = useTranscriptPlaybackStore;
  window.useTranscriptRuntimeStore = useTranscriptRuntimeStore;
  window.useTranscriptSidecarStore = useTranscriptSidecarStore;
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
