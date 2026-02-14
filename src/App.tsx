import React, { useState } from 'react';
import { ErrorBoundary } from './components/ErrorBoundary';
import { useTranslation } from 'react-i18next';
import './styles/index.css';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptEditor } from './components/TranscriptEditor';
import { AudioPlayer } from './components/AudioPlayer';
import { ExportButton } from './components/ExportButton';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { HistoryView } from './components/HistoryView';
import { Settings } from './components/Settings';
import { GlobalDialog } from './components/GlobalDialog';
import { useTranscriptStore } from './stores/transcriptStore';
import { SettingsIcon, WaveformIcon } from './components/Icons';
import { useAppInitialization } from './hooks/useAppInitialization';
import { useAutoSaveTranscript } from './hooks/useAutoSaveTranscript';

/**
 * Main application component.
 *
 * Handles layout, initialization of config, theme, and language.
 *
 * @return The root application element.
 */
function App(): React.JSX.Element {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const mode = useTranscriptStore((state) => state.mode);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  const { t } = useTranslation();

  // Run application initialization logic
  useAppInitialization();

  // Initialize auto-save hook
  useAutoSaveTranscript();

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

      {/* Main Content */}
      <main id="main-content" className="app-main">
        <div className="panel-container">
          {/* Left Panel - Input */}
          <div className="panel panel-left">
            <div className="panel-header">
              <h2>{mode === 'live' ? t('panel.live_record') : mode === 'history' ? t('history.title') : t('panel.batch_import')}</h2>
            </div>
            <div className="panel-content">
              {mode === 'live' ? <LiveRecord /> : mode === 'history' ? <HistoryView /> : <BatchImport />}
            </div>
          </div>

          {/* Right Panel - Editor */}
          <div className="panel panel-right">

            <div className="panel-content">
              <ErrorBoundary>
                <TranscriptEditor />
              </ErrorBoundary>
            </div>
            {audioUrl && <AudioPlayer />}
          </div>
        </div>
      </main>

      {/* Settings Modal */}
      <Settings isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />
      <GlobalDialog />
    </div>
  );
}

export default App;
