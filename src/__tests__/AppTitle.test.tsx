
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import App from '../App';

// Mock all subcomponents to isolate App logic
vi.mock('../components/ErrorBoundary', () => ({ ErrorBoundary: ({ children }: any) => <div>{children}</div> }));
vi.mock('../components/TabNavigation', () => ({ TabNavigation: () => <div>TabNavigation</div> }));
vi.mock('../components/TranscriptEditor', () => ({ TranscriptEditor: () => <div>TranscriptEditor</div> }));
vi.mock('../components/AudioPlayer', () => ({ AudioPlayer: () => <div>AudioPlayer</div> }));
vi.mock('../components/ExportButton', () => ({ ExportButton: () => <div>ExportButton</div> }));
vi.mock('../components/TranslateButton', () => ({ TranslateButton: () => <div>TranslateButton</div> }));
vi.mock('../components/PolishButton', () => ({ PolishButton: () => <div>PolishButton</div> }));
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/HistoryView', () => ({ HistoryView: () => <div>HistoryView</div> }));
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/ProjectContextBar', () => ({ ProjectContextBar: () => <div>ProjectContextBar</div> }));
vi.mock('../components/Settings', () => ({ Settings: () => <div>Settings</div> }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => <div>OnboardingReminderBanner</div> }));
vi.mock('../components/Icons', () => ({ SettingsIcon: () => <span>SettingsIcon</span>, WaveformIcon: () => <span>WaveformIcon</span> }));

// Mock hooks
vi.mock('../hooks/useAppInitialization', () => ({ 
  useAppInitialization: () => ({ isLoaded: true }) 
}));
vi.mock('../hooks/useAutoSaveTranscript', () => ({ useAutoSaveTranscript: vi.fn() }));
vi.mock('../hooks/useTrayHandling', () => ({ useTrayHandling: vi.fn() }));

// Mock stores
const mockUseTranscriptStore = vi.fn();
vi.mock('../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => mockUseTranscriptStore(selector)
}));

// Mock translation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('App Title Logic', () => {
  const defaultState = {
    mode: 'live',
    audioUrl: null,
    config: { theme: 'light' }
  };

  const setupStore = (overrides = {}) => {
    const state = { ...defaultState, ...overrides };
    mockUseTranscriptStore.mockImplementation((selector: any) => selector(state));
  };

  it('displays "Live Record" title in live mode', () => {
    setupStore({ mode: 'live' });
    render(<App />);
    expect(screen.getByText('panel.live_record')).not.toBeNull();
  });

  it('displays "History" title in history mode', () => {
    setupStore({ mode: 'history' });
    render(<App />);
    expect(screen.getByText('history.title')).not.toBeNull();
  });

  it('displays "Batch Import" title in batch mode', () => {
    setupStore({ mode: 'batch' });
    render(<App />);
    expect(screen.getByText('panel.batch_import')).not.toBeNull();
  });

  it('renders the projects workbench without the old editor shell in projects mode', () => {
    setupStore({ mode: 'projects' });
    render(<App />);
    expect(screen.getByText('ProjectsView')).not.toBeNull();
    expect(screen.queryByText('TranscriptEditor')).toBeNull();
  });
});
