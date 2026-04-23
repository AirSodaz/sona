import React, { useState } from 'react';
import { ErrorBoundary } from './components/ErrorBoundary';
import { useTranslation } from 'react-i18next';
import './styles/index.css';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptEditor } from './components/TranscriptEditor';
import { AudioPlayer } from './components/AudioPlayer';
import { ExportButton } from './components/ExportButton';
import { TranslateButton } from './components/TranslateButton';
import { PolishButton } from './components/PolishButton';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { HistoryView } from './components/HistoryView';
import { ProjectsView } from './components/ProjectsView';
import { ProjectContextBar } from './components/ProjectContextBar';
import { Settings } from './components/Settings';
import { GlobalDialog } from './components/GlobalDialog';
import { ErrorDialog } from './components/ErrorDialog';
import { FirstRunGuide } from './components/FirstRunGuide';
import { OnboardingReminderBanner } from './components/OnboardingReminderBanner';
import { UpdateNotification } from './components/UpdateNotification';
// import { LiveCaptionOverlay } from './components/LiveCaptionOverlay';
import { useTranscriptStore } from './stores/transcriptStore';
import { SettingsIcon, WaveformIcon } from './components/Icons';
import { useAppInitialization } from './hooks/useAppInitialization';
import { useAutoSaveTranscript } from './hooks/useAutoSaveTranscript';
import { useTrayHandling } from './hooks/useTrayHandling';
import { useTranscriptionServiceSync } from './hooks/useTranscriptionServiceSync';
import { SettingsTab } from './hooks/useSettingsLogic';

/**
 * Helper to determine the title of the left panel based on the current mode.
 */
function getPanelTitle(mode: string, t: (key: string) => string): string {
  switch (mode) {
    case 'live':
      return t('panel.live_record');
    case 'history':
      return t('history.title');
    case 'projects':
      return t('panel.projects');
    case 'batch':
    default:
      return t('panel.batch_import');
  }
}

function App(): React.JSX.Element {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>('general');
  const mode = useTranscriptStore((state) => state.mode);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  // const isRecording = useTranscriptStore((state) => state.isRecording);
  // const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);
  const { t } = useTranslation();

  // Run application initialization logic
  const { isLoaded } = useAppInitialization();

  // Initialize auto-save hook
  useAutoSaveTranscript();

  // Keep TranscriptionService synced and preloaded in background
  useTranscriptionServiceSync();

  // Handle tray events
  useTrayHandling(setIsSettingsOpen, setSettingsInitialTab);

  if (!isLoaded) {
    return <></>; // Wait for config and onboarding state to load
  }

  const panelTitle = getPanelTitle(mode, t);

  return (
    <div className="app">
      <a href="#main-content" className="skip-link">{t('common.skip_to_content')}</a>
      {/* Header */}
      <header className="app-header">
        <div className="app-logo">
          <WaveformIcon />
          <h1>Sona</h1>
        </div>

        <TabNavigation />

        <div className="header-actions">
          <PolishButton />
          <TranslateButton />
          <ExportButton />
          <button
            className="btn btn-icon"
            onClick={() => setIsSettingsOpen(true)}
            data-tooltip={t('header.settings')}
            data-tooltip-pos="bottom-left"
            aria-label={t('header.settings')}
          >
            <SettingsIcon />
          </button>
        </div>
      </header>

      <OnboardingReminderBanner />

      {/* Main Content */}
      <main id="main-content" className="app-main">
        <div className="panel-container">
          {/* Left Panel - Input */}
          <div className="panel panel-left">
            <div className="panel-header">
              <h2>{panelTitle}</h2>
            </div>
            <div className="panel-content">
              {mode !== 'projects' && <ProjectContextBar />}
              <div style={{ display: mode === 'live' ? undefined : 'none', height: '100%' }}>
                <LiveRecord />
              </div>
              {mode === 'history' && <HistoryView />}
              {mode === 'batch' && <BatchImport />}
              {mode === 'projects' && <ProjectsView />}
            </div>
          </div>

          {/* Right Panel - Editor */}
          <div className="panel panel-right">

            <div className="panel-content">
              <ProjectContextBar />
              <ErrorBoundary>
                <TranscriptEditor />
              </ErrorBoundary>
            </div>
            {audioUrl && <AudioPlayer />}
          </div>
        </div>
      </main>

      {/* Live Caption Overlay - rendered at app level to survive tab switches */}
      {/* {isCaptionMode && isRecording && <LiveCaptionOverlay />} */}

      {/* Settings Modal */}
      <Settings isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} initialTab={settingsInitialTab} />
      <GlobalDialog />
      <ErrorDialog />
      <FirstRunGuide />
      <UpdateNotification />
    </div>
  );
}

export default App;
