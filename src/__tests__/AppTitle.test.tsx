
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
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/Settings', () => ({ Settings: () => <div>Settings</div> }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => <div>OnboardingReminderBanner</div> }));
vi.mock('../components/UpdateNotification', () => ({ UpdateNotification: () => <div>UpdateNotification</div> }));
vi.mock('../components/Icons', () => ({ SettingsIcon: () => <span>SettingsIcon</span>, CloseIcon: () => <span>CloseIcon</span> }));

// Mock hooks
vi.mock('../hooks/useAppInitialization', () => ({ 
  useAppInitialization: () => ({ isLoaded: true }) 
}));
vi.mock('../hooks/useAutoSaveTranscript', () => ({ useAutoSaveTranscript: vi.fn() }));
vi.mock('../hooks/useAutoUpdateCheck', () => ({ useAutoUpdateCheck: vi.fn() }));
vi.mock('../hooks/useTrayHandling', () => ({ useTrayHandling: vi.fn() }));
vi.mock('../hooks/useTranscriptionServiceSync', () => ({ useTranscriptionServiceSync: vi.fn() }));

// Mock stores
const mockUseTranscriptStore = vi.fn();
vi.mock('../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => mockUseTranscriptStore(selector)
}));

const mockUseProjectStore = vi.fn();
vi.mock('../stores/projectStore', () => ({
  useProjectStore: (selector: any) => mockUseProjectStore(selector)
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
    config: { theme: 'light' },
    segments: [],
    clearSegments: vi.fn(),
    setMode: vi.fn(),
    title: ''
  };

  const defaultProjectState = {
    activeProjectId: null,
    projects: []
  };

  const setupStore = (overrides = {}, projectOverrides = {}) => {
    const state = { ...defaultState, ...overrides };
    mockUseTranscriptStore.mockImplementation((selector: any) => selector(state));
    
    const projectState = { ...defaultProjectState, ...projectOverrides };
    mockUseProjectStore.mockImplementation((selector: any) => selector(projectState));
  };

  it('displays "Live Record" title in live mode', () => {
    setupStore({ mode: 'live' });
    render(<App />);
    expect(screen.getByText('panel.live_record')).not.toBeNull();
  });

  it('displays "Batch Import" title in batch mode', () => {
    setupStore({ mode: 'batch' });
    render(<App />);
    expect(screen.getByText('panel.batch_import')).not.toBeNull();
  });

  it('displays active project name as a tag in header when active', () => {
    setupStore({ mode: 'live' }, { 
      activeProjectId: 'p1', 
      projects: [{ id: 'p1', name: 'My Project' }] 
    });
    render(<App />);
    expect(screen.getByText('My Project')).not.toBeNull();
    // Verify it's in the app-logo section (implicitly by checking no other My Project text)
    expect(screen.queryByText('ProjectContextBar')).toBeNull();
  });

  it('renders the projects workbench without the old editor shell in projects mode', () => {
    setupStore({ mode: 'projects' });
    render(<App />);
    expect(screen.getByText('ProjectsView')).not.toBeNull();
    expect(screen.queryByText('TranscriptEditor')).toBeNull();
  });
});
