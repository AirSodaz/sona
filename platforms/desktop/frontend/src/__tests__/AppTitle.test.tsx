import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';

vi.mock('react-i18next', async () => {
  const { createReactI18nextMock } = await import('./testUtils/i18n');
  return createReactI18nextMock();
});

vi.mock('../components/TabNavigation', () => ({ TabNavigation: () => <div>TabNavigation</div> }));
vi.mock('../components/transcript/TranscriptWorkbench', () => ({
  TranscriptWorkbench: () => <div>TranscriptWorkbench</div>,
}));
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/tools/ToolsView', () => ({ ToolsView: () => <div>ToolsView</div> }));
vi.mock('../components/Settings', () => ({ Settings: () => <div>Settings</div> }));
vi.mock('../components/DiagnosticsModal', () => ({
  DiagnosticsModal: () => <div>DiagnosticsModal</div>,
}));
vi.mock('../components/RecoveryCenterModal', () => ({
  RecoveryCenterModal: () => <div>RecoveryCenterModal</div>,
}));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/NotificationCenter', () => ({ NotificationCenter: () => null }));
vi.mock('../components/OnboardingReminderBanner', () => ({
  OnboardingReminderBanner: () => <div>OnboardingReminderBanner</div>,
}));
vi.mock('../components/Icons', async (importOriginal) => {
  const { buildPartialIconsMock, createNamedIconMock } = await import('./testUtils/icons');
  return buildPartialIconsMock(() => importOriginal<typeof import('../components/Icons')>(), {
    AutomationIcon: createNamedIconMock('AutomationIcon'),
    SettingsIcon: createNamedIconMock('SettingsIcon'),
  });
});

vi.mock('../hooks/useAppInitialization', () => ({
  useAppInitialization: () => ({ isLoaded: true }),
}));
vi.mock('../hooks/useAutoSaveTranscript', () => ({ useAutoSaveTranscript: vi.fn() }));
vi.mock('../hooks/useAutoUpdateCheck', () => ({ useAutoUpdateCheck: vi.fn() }));
vi.mock('../hooks/useTrayHandling', () => ({ useTrayHandling: vi.fn() }));
vi.mock('../hooks/useTranscriptionServiceSync', () => ({ useTranscriptionServiceSync: vi.fn() }));

const mockUseTranscriptRuntimeStore = vi.fn();
const mockUseProjectStore = vi.fn();
const mockUseOnboardingStore = vi.fn();

vi.mock('../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: (selector: (state: unknown) => unknown) =>
    mockUseTranscriptRuntimeStore(selector),
}));

vi.mock('../stores/projectStore', () => ({
  useProjectStore: (selector: (state: unknown) => unknown) => mockUseProjectStore(selector),
}));

vi.mock('../stores/onboardingStore', () => ({
  useOnboardingStore: (selector: (state: unknown) => unknown) => mockUseOnboardingStore(selector),
}));

describe('App Title Logic', () => {
  const defaultTranscriptState = {
    mode: 'create',
    ingestionFocus: 'live',
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
    onboardingOverrides = {}
  ) => {
    const transcriptState = { ...defaultTranscriptState, ...transcriptOverrides };
    const projectState = { ...defaultProjectState, ...projectOverrides };
    const onboardingState = { ...defaultOnboardingState, ...onboardingOverrides };

    mockUseTranscriptRuntimeStore.mockImplementation(
      (selector: (state: typeof transcriptState) => unknown) => selector(transcriptState)
    );
    mockUseProjectStore.mockImplementation((selector: (state: typeof projectState) => unknown) =>
      selector(projectState)
    );
    mockUseOnboardingStore.mockImplementation(
      (selector: (state: typeof onboardingState) => unknown) => selector(onboardingState)
    );
  };

  beforeEach(() => {
    vi.clearAllMocks();
    setupStore();
  });

  it('renders segmented control without redundant heading in create mode', () => {
    render(<App />);
    expect(screen.queryByRole('heading', { level: 2, name: 'panel.create' })).toBeNull();
    expect(screen.getByRole('tablist')).not.toBeNull();
  });

  it('renders the tools workbench and shows tools shell in tools mode', () => {
    setupStore({ mode: 'tools' });
    const { container } = render(<App />);

    expect(screen.getByText('ToolsView')).not.toBeNull();
    const toolsShell = container.querySelector('.tools-mode-shell') as HTMLElement;
    expect(toolsShell).not.toBeNull();
    expect(toolsShell.style.display).not.toBe('none');

    const workspaceShell = container.querySelector('.workspace-mode-shell') as HTMLElement;
    expect(workspaceShell.style.display).toBe('none');
  });

  it('renders the projects workbench and shows projects shell in projects mode', () => {
    setupStore({ mode: 'projects' });
    const { container } = render(<App />);

    expect(screen.getByText('ProjectsView')).not.toBeNull();
    const projectsShell = container.querySelector('.projects-mode-shell') as HTMLElement;
    expect(projectsShell).not.toBeNull();
    expect(projectsShell.style.display).not.toBe('none');

    const workspaceShell = container.querySelector('.workspace-mode-shell') as HTMLElement;
    expect(workspaceShell.style.display).toBe('none');
  });
  it('displays active project name as a tag in header when active', () => {
    setupStore(
      { mode: 'create' },
      {
        activeProjectId: 'p1',
        projects: [{ id: 'p1', name: 'My Project' }],
      }
    );

    render(<App />);

    expect(screen.getByText('My Project')).not.toBeNull();
    expect(screen.queryByText('ProjectContextBar')).toBeNull();
  });

  it('renders the projects workbench and keeps the workspace shell hidden in projects mode', () => {
    setupStore({ mode: 'projects' });

    const { container } = render(<App />);

    expect(screen.getByText('ProjectsView')).not.toBeNull();
    expect(screen.getByText('TranscriptWorkbench')).not.toBeNull();
    expect(
      container.querySelector('.persistent-transcript-host')?.classList.contains('is-hidden')
    ).toBe(true);

    const workspaceShell = container.querySelector('.workspace-mode-shell');
    expect((workspaceShell as HTMLElement).style.display).toBe('none');
  });
});
