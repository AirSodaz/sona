import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import './styles/index.css';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptEditor } from './components/TranscriptEditor';
import { AudioPlayer, seekAudio } from './components/AudioPlayer';
import { ExportButton } from './components/ExportButton';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { Settings } from './components/Settings';
import { useTranscriptStore } from './stores/transcriptStore';

// Icons
const SettingsIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

const WaveformIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M2 12h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H2z" />
    <path d="M8 6h2a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8z" />
    <path d="M14 4h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-2z" />
    <path d="M20 10h2a2 2 0 0 1 2 2v0a2 2 0 0 1-2 2h-2z" />
  </svg>
);

function App() {
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
    const applyTheme = () => {
      const theme = config.theme || 'auto';
      const root = document.documentElement;

      if (theme === 'dark') {
        root.setAttribute('data-theme', 'dark');
      } else if (theme === 'light') {
        root.setAttribute('data-theme', 'light');
      } else {
        // Auto
        root.removeAttribute('data-theme');
        // We rely on media query for auto, OR we can explicit set it to match system
        // The plan said: Let JS set the attribute based on preference OR just use media query.
        // If index.css uses [data-theme="dark"] AND media query, we might not need to do anything for auto if we remove attribute.
        // BUT, if we want to force light in auto mode when system is light, removing attribute is fine IF defaults are light.
        // Let's verify index.css strategy.
        // If we remove attribute, it falls back to :root (light) and @media (dark).
        // So for Auto, removing attribute is correct.
      }
    };

    applyTheme();
  }, [config.theme]);

  // Apply font
  useEffect(() => {
    const font = config.font || 'system';
    const root = document.documentElement;

    if (font === 'system') {
      root.style.removeProperty('--font-sans');
    } else if (font === 'serif') {
      root.style.setProperty('--font-sans', 'Merriweather, serif');
    } else if (font === 'sans') {
      root.style.setProperty('--font-sans', 'Inter, sans-serif');
    } else if (font === 'mono') {
      root.style.setProperty('--font-sans', 'JetBrains Mono, monospace');
    } else if (font === 'arial') {
      root.style.setProperty('--font-sans', 'Arial, sans-serif');
    } else if (font === 'georgia') {
      root.style.setProperty('--font-sans', 'Georgia, serif');
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
    </div>
  );
}

export default App;
