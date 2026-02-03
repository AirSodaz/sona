import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from './i18n';
import './styles/index.css';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptEditor } from './components/TranscriptEditor';
import { AudioPlayer, seekAudio } from './components/AudioPlayer';
import { ExportButton } from './components/ExportButton';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { Settings } from './components/Settings';
import { GlobalDialog } from './components/GlobalDialog';
import { useTranscriptStore } from './stores/transcriptStore';

// Icons
function SettingsIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function WaveformIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2 12h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H2z" />
      <path d="M8 6h2a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8z" />
      <path d="M14 4h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-2z" />
      <path d="M20 10h2a2 2 0 0 1 2 2v0a2 2 0 0 1-2 2h-2z" />
    </svg>
  );
}

/**
 * Main application component.
 * Handles layout, initialization of config, theme, and language.
 *
 * @return The root application element.
 */
function App(): React.JSX.Element {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const mode = useTranscriptStore((state) => state.mode);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  const { t } = useTranslation();

  const handleSeek = (time: number) => {
    seekAudio(time);
  };

  // Apply theme
  const config = useTranscriptStore((state) => state.config);

  useEffect(() => {
    // Hydrate config from localStorage
    const saved = localStorage.getItem('sona-config');
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        if (parsed.streamingModelPath || parsed.offlineModelPath || parsed.modelPath || parsed.appLanguage) {
          const setConfig = useTranscriptStore.getState().setConfig;

          // Legacy support for 'modelPath'
          const legacyPath = parsed.modelPath || '';

          setConfig({
            streamingModelPath: parsed.streamingModelPath || legacyPath,
            offlineModelPath: parsed.offlineModelPath || '',
            punctuationModelPath: parsed.punctuationModelPath || '',
            vadModelPath: parsed.vadModelPath || '',
            enabledITNModels: parsed.enabledITNModels || (parsed.enableITN ? ['itn-zh-number'] : []),
            itnRulesOrder: parsed.itnRulesOrder || ['itn-zh-number'],
            vadBufferSize: parsed.vadBufferSize || 5,
            appLanguage: parsed.appLanguage || 'auto',
            theme: parsed.theme || 'auto',
            font: parsed.font || 'system'
          });

          // Apply language immediately
          if (parsed.appLanguage && parsed.appLanguage !== 'auto') {
            i18n.changeLanguage(parsed.appLanguage);
          } else {
            i18n.changeLanguage(navigator.language);
          }
        }
      } catch (e) {
        console.error('Failed to parse saved config:', e);
      }
    }
  }, []);

  useEffect(() => {
    const applyTheme = () => {
      const theme = config.theme || 'auto';
      const root = document.documentElement;

      if (theme === 'dark') {
        root.setAttribute('data-theme', 'dark');
      } else if (theme === 'light') {
        root.setAttribute('data-theme', 'light');
      } else {
        root.removeAttribute('data-theme');
      }
    };

    applyTheme();
  }, [config.theme]);

  // Apply font
  useEffect(() => {
    const font = config.font || 'system';
    const root = document.documentElement;

    switch (font) {
      case 'serif':
        root.style.setProperty('--font-sans', 'Merriweather, serif');
        break;
      case 'sans':
        root.style.setProperty('--font-sans', 'Inter, sans-serif');
        break;
      case 'mono':
        root.style.setProperty('--font-sans', 'JetBrains Mono, monospace');
        break;
      case 'arial':
        root.style.setProperty('--font-sans', 'Arial, sans-serif');
        break;
      case 'georgia':
        root.style.setProperty('--font-sans', 'Georgia, serif');
        break;
      case 'system':
      default:
        root.style.removeProperty('--font-sans');
        break;
    }
  }, [config.font]);

  return (
    <div className="app">
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
      <main className="app-main">
        <div className="panel-container">
          {/* Left Panel - Input */}
          <div className="panel panel-left">
            <div className="panel-header">
              <h2>{mode === 'live' ? t('panel.live_record') : t('panel.batch_import')}</h2>
            </div>
            <div className="panel-content">
              {mode === 'live' ? <LiveRecord /> : <BatchImport />}
            </div>
          </div>

          {/* Right Panel - Editor */}
          <div className="panel panel-right">
            <div className="panel-header">
              <h2>{t('panel.transcript')}</h2>
            </div>
            <div className="panel-content">
              <TranscriptEditor onSeek={handleSeek} />
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
