import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './i18n';
import './styles/index.css';
import { CaptionWindow } from './components/CaptionWindow';
import { ContextMenuProvider } from './components/context-menu/ContextMenuProvider';
import { ErrorBoundary } from './components/ErrorBoundary';
import { RemoteWebEditor } from './components/RemoteWebEditor';
import { VoiceTypingOverlay } from './components/VoiceTypingOverlay';
import { modelService } from './services/modelService';
import { transcriptionService } from './services/transcriptionService';
import { voiceTypingService } from './services/voiceTypingService';
import { useBatchQueueStore } from './stores/batchQueueStore';
import { useDialogStore } from './stores/dialogStore';
import { useOnboardingStore } from './stores/onboardingStore';
import { useTranscriptPlaybackStore } from './stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from './stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from './stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from './stores/transcriptSidecarStore';
import { logger } from './utils/logger';

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

// Global error handlers — log unhandled errors for diagnostics
window.addEventListener('unhandledrejection', (event) => {
  logger.error('[Global] Unhandled promise rejection:', event.reason);
});
window.addEventListener('error', (event) => {
  logger.error('[Global] Uncaught error:', event.error);
});

const isCaptionWindow = window.location.search.includes('window=caption');
const isVoiceTypingWindow = window.location.search.includes('window=voice-typing');
const isTauri =
  typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
const isExplicitWebMode =
  window.location.search.includes('mode=web') || window.location.pathname.startsWith('/web');
const isExplicitAppMode = window.location.search.includes('mode=app');
const isDevFrontendPort =
  window.location.port === '1420' ||
  window.location.port === '5173' ||
  window.location.port === '5174';

const isWebMode =
  !isExplicitAppMode &&
  (isExplicitWebMode || import.meta.env.MODE === 'web' || (!isTauri && !isDevFrontendPort));

let rootComponent = <App />;
if (isVoiceTypingWindow) {
  rootComponent = <VoiceTypingOverlay />;
} else if (isCaptionWindow) {
  rootComponent = <CaptionWindow />;
} else if (isWebMode) {
  rootComponent = <RemoteWebEditor />;
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ContextMenuProvider>{rootComponent}</ContextMenuProvider>
    </ErrorBoundary>
  </React.StrictMode>
);
