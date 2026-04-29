import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import App from '../App';

vi.mock('react-i18next', async () => {
  const { createReactI18nextMock } = await import('./testUtils/i18n');
  return createReactI18nextMock();
});

vi.mock('../components/TabNavigation', () => ({ TabNavigation: () => <div>TabNavigation</div> }));
vi.mock('../components/TranscriptWorkbench', () => ({ TranscriptWorkbench: () => <div>TranscriptWorkbench</div> }));
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/Settings', () => ({ Settings: () => <div>Settings</div> }));
vi.mock('../components/DiagnosticsModal', () => ({ DiagnosticsModal: () => <div>DiagnosticsModal</div> }));
vi.mock('../components/RecoveryCenterModal', () => ({ RecoveryCenterModal: () => <div>RecoveryCenterModal</div> }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/NotificationCenter', () => ({ NotificationCenter: () => null }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => <div>OnboardingReminderBanner</div> }));
vi.mock('../components/Icons', async (importOriginal) => {
  const { buildPartialIconsMock, createNamedIconMock } = await import('./testUtils/icons');
  return buildPartialIconsMock(
    () => importOriginal<typeof import('../components/Icons')>(),
    {
      AutomationIcon: createNamedIconMock('AutomationIcon'),
      SettingsIcon: createNamedIconMock('SettingsIcon'),
    },
  );
});

vi.mock('../hooks/useAppInitialization', () => ({
  useAppInitialization: () => ({ isLoaded: true }),
}));
vi.mock('../hooks/useAutoSaveTranscript', () => ({ useAutoSaveTranscript: vi.fn() }));
vi.mock('../hooks/useAutoUpdateCheck', () => ({ useAutoUpdateCheck: vi.fn() }));
vi.mock('../hooks/useTrayHandling', () => ({ useTrayHandling: vi.fn() }));
vi.mock('../hooks/useTranscriptionServiceSync', () => ({ useTranscriptionServiceSync: vi.fn() }));

const mockUseTranscriptStore = vi.fn();
const mockUseProjectStore = vi.fn();
const mockUseOnboardingStore = vi.fn();

vi.mock('../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: (state: unknown) => unknown) => mockUseTranscriptStore(selector),
}));

vi.mock('../stores/projectStore', () => ({
  useProjectStore: (selector: (state: unknown) => unknown) => mockUseProjectStore(selector),
}));

vi.mock('../stores/onboardingStore', () => ({
  useOnboardingStore: (selector: (state: unknown) => unknown) => mockUseOnboardingStore(selector),
}));

describe('App Title Logic', () => {
  const defaultTranscriptState = {
    mode: 'live',
    clearSegments: vi.fn(),
    setMode: vi.fn(),
  };

  const defaultProjectState = {
    activeProjectId: null,
    projects: [],
  };

  const defaultOnboardingState = {
    reopen: vi.fn(),
  };

  const setupStore = (
    transcriptOverrides = {},
    projectOverrides = {},
    onboardingOverrides = {},
  ) => {
    const transcriptState = { ...defaultTranscriptState, ...transcriptOverrides };
    const projectState = { ...defaultProjectState, ...projectOverrides };
    const onboardingState = { ...defaultOnboardingState, ...onboardingOverrides };

    mockUseTranscriptStore.mockImplementation((selector: (state: typeof transcriptState) => unknown) => selector(transcriptState));
    mockUseProjectStore.mockImplementation((selector: (state: typeof projectState) => unknown) => selector(projectState));
    mockUseOnboardingStore.mockImplementation((selector: (state: typeof onboardingState) => unknown) => selector(onboardingState));
  };

  beforeEach(() => {
    vi.clearAllMocks();
    setupStore();
  });

  it('displays "Live Record" title in live mode', () => {
    render(<App />);
    expect(screen.getByText('panel.live_record')).not.toBeNull();
  });

  it('displays "Batch Import" title in batch mode', () => {
    setupStore({ mode: 'batch' });
    render(<App />);
    expect(screen.getByText('panel.batch_import')).not.toBeNull();
  });

  it('displays active project name as a tag in header when active', () => {
    setupStore(
      { mode: 'live' },
      {
        activeProjectId: 'p1',
        projects: [{ id: 'p1', name: 'My Project', defaults: { summaryTemplateId: 'general' } }],
      },
    );

    render(<App />);

    expect(screen.getByText('My Project')).not.toBeNull();
    expect(screen.queryByText('ProjectContextBar')).toBeNull();
  });

  it('renders the projects workbench and keeps the editor shell in the DOM (hidden) in projects mode', () => {
    setupStore({ mode: 'projects' });

    const { container } = render(<App />);

    expect(screen.getByText('ProjectsView')).not.toBeNull();
    expect(screen.getByText('TranscriptWorkbench')).not.toBeNull();

    const workspaceShell = container.querySelector('.workspace-mode-shell');
    expect((workspaceShell as HTMLElement).style.display).toBe('none');
  });
});
