import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import App from '../App';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';

const settingsModuleLoaded = vi.hoisted(() => vi.fn());
const preloadSettingsTabMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
const preloadAllSettingsTabsMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
const markSettingsPerfMock = vi.hoisted(() => vi.fn());
const projectsViewRenderMock = vi.hoisted(() => vi.fn());
const transcriptWorkbenchMountMock = vi.hoisted(() => vi.fn());
const transcriptWorkbenchUnmountMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../components/TabNavigation', () => ({ TabNavigation: () => <div>TabNavigation</div> }));
vi.mock('../components/transcript/TranscriptWorkbench', async () => {
  const React = await import('react');

  return {
    TranscriptWorkbench: ({ onClose }: { onClose: () => void }) => {
      React.useEffect(() => {
        transcriptWorkbenchMountMock();
        return () => transcriptWorkbenchUnmountMock();
      }, []);

      return (
        <div data-testid="transcript-workbench">
          TranscriptWorkbench
          <button type="button" onClick={onClose}>Close Transcript</button>
        </div>
      );
    },
  };
});
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/ProjectsView', () => ({
  ProjectsView: (props: any) => {
    projectsViewRenderMock(props);
    return <div data-testid="projects-view">ProjectsView: {String(props.isActive)}</div>;
  },
}));
vi.mock('../components/DiagnosticsModal', () => ({ DiagnosticsModal: () => null }));
vi.mock('../components/RecoveryCenterModal', () => ({ RecoveryCenterModal: () => null }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/NotificationCenter', () => ({
  NotificationCenter: ({ onOpenAutomationSettings }: any) => (
    <button type="button" onClick={onOpenAutomationSettings}>
      Open Automation Settings
    </button>
  ),
}));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => null }));
vi.mock('../components/Icons', () => ({
  AutomationIcon: () => <span>AutomationIcon</span>,
  SettingsIcon: () => <span>SettingsIcon</span>,
}));

vi.mock('../components/settings/settingsLoaders', () => ({
  preloadSettingsTab: preloadSettingsTabMock,
  preloadAllSettingsTabs: preloadAllSettingsTabsMock,
}));

vi.mock('../utils/settingsPerf', () => ({
  getSettingsPerfErrorDetail: (error: unknown) => ({
    message: error instanceof Error ? error.message : String(error),
  }),
  markSettingsPerf: markSettingsPerfMock,
}));

vi.mock('../components/Settings', () => {
  settingsModuleLoaded();

  return {
    Settings: ({ isOpen, prewarm, initialTab, onClose }: any) => (
      isOpen ? (
        <div>
          <div>Settings Tab: {initialTab}</div>
          <button type="button" onClick={onClose}>Close Settings</button>
        </div>
      ) : prewarm ? (
        <div data-testid="settings-prewarm">Settings Prewarm: {initialTab}</div>
      ) : null
    ),
  };
});

vi.mock('../hooks/useAppInitialization', () => ({
  useAppInitialization: () => ({ isLoaded: true }),
}));
vi.mock('../hooks/useAutoSaveTranscript', () => ({ useAutoSaveTranscript: vi.fn() }));
vi.mock('../hooks/useAutoUpdateCheck', () => ({ useAutoUpdateCheck: vi.fn() }));
vi.mock('../hooks/useTrayHandling', () => ({ useTrayHandling: vi.fn() }));
vi.mock('../hooks/useTranscriptionServiceSync', () => ({ useTranscriptionServiceSync: vi.fn() }));
vi.mock('../services/diagnosticsService', () => ({
  diagnosticsService: {
    getResumeOnboardingStep: vi.fn(() => 'welcome'),
  },
}));

const mockUseTranscriptRuntimeStore = vi.fn();
const mockUseProjectStore = vi.fn();
const mockUseOnboardingStore = vi.fn();

vi.mock('../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: (selector: any) => mockUseTranscriptRuntimeStore(selector),
}));

vi.mock('../stores/projectStore', () => ({
  useProjectStore: (selector: any) => mockUseProjectStore(selector),
}));

vi.mock('../stores/onboardingStore', () => ({
  useOnboardingStore: (selector: any) => mockUseOnboardingStore(selector),
}));

describe('App settings preload', () => {
  beforeEach(() => {
    settingsModuleLoaded.mockClear();
    preloadSettingsTabMock.mockClear();
    preloadAllSettingsTabsMock.mockClear();
    markSettingsPerfMock.mockClear();
    projectsViewRenderMock.mockClear();
    transcriptWorkbenchMountMock.mockClear();
    transcriptWorkbenchUnmountMock.mockClear();
    useTranscriptSessionStore.getState().clearSegments();
    useTranscriptPlaybackStore.getState().clearSession({ clearAudio: true });
    mockUseTranscriptRuntimeStore.mockImplementation((selector: any) => selector({
      mode: 'live',
      setMode: vi.fn(),
    }));
    mockUseProjectStore.mockImplementation((selector: any) => selector({
      activeProjectId: null,
      projects: [],
    }));
    mockUseOnboardingStore.mockImplementation((selector: any) => selector({
      reopen: vi.fn(),
    }));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('preloads settings from the header button without rendering a loading overlay', async () => {
    render(<App />);

    const settingsButton = screen.getByRole('button', { name: 'header.settings' });

    await waitFor(() => {
      expect(settingsModuleLoaded).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(preloadAllSettingsTabsMock).toHaveBeenCalledTimes(1);
    });
    expect(markSettingsPerfMock).toHaveBeenCalledWith('settings.preload.all.start');
    await waitFor(() => {
      expect(markSettingsPerfMock).toHaveBeenCalledWith('settings.preload.all.end');
    });
    await waitFor(() => {
      expect(screen.getByTestId('settings-prewarm').textContent).toBe('Settings Prewarm: general');
    });
    expect(markSettingsPerfMock).toHaveBeenCalledWith('settings.prewarm.hidden.request');

    fireEvent.pointerEnter(settingsButton);
    await waitFor(() => {
      expect(settingsModuleLoaded).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(preloadSettingsTabMock).toHaveBeenCalledWith('general');
    });

    fireEvent.focus(settingsButton);
    await waitFor(() => {
      expect(preloadSettingsTabMock).toHaveBeenCalledTimes(2);
    });

    fireEvent.click(settingsButton);

    expect(markSettingsPerfMock).toHaveBeenCalledWith(
      'settings.open.default.click',
      { tab: 'general', source: 'header' },
    );
    expect(await screen.findByText('Settings Tab: general')).toBeDefined();
    expect(screen.queryByTestId('settings-prewarm')).toBeNull();
    expect(screen.queryByRole('dialog', { name: 'common.loading' })).toBeNull();

    fireEvent.click(screen.getByText('Open Automation Settings'));
    await waitFor(() => {
      expect(preloadSettingsTabMock).toHaveBeenCalledWith('automation');
    });
    expect(markSettingsPerfMock).toHaveBeenCalledWith(
      'settings.open.tab.click',
      { tab: 'automation' },
    );
    expect(await screen.findByText('Settings Tab: automation')).toBeDefined();

    fireEvent.click(screen.getByText('Close Settings'));
    await waitFor(() => {
      expect(screen.queryByText('Settings Tab: automation')).toBeNull();
    });
    expect(screen.getByTestId('settings-prewarm').textContent).toBe('Settings Prewarm: general');

    fireEvent.click(settingsButton);
    expect(await screen.findByText('Settings Tab: general')).toBeDefined();
  });

  it('passes the active mode flag into the kept-mounted projects view', () => {
    render(<App />);

    expect(screen.getByTestId('projects-view').textContent).toBe('ProjectsView: false');
    expect(projectsViewRenderMock).toHaveBeenLastCalledWith(expect.objectContaining({ isActive: false }));

    mockUseTranscriptRuntimeStore.mockImplementation((selector: any) => selector({
      mode: 'projects',
      setMode: vi.fn(),
    }));

    render(<App />);

    const renderedProjectViews = screen.getAllByTestId('projects-view');
    expect(renderedProjectViews[renderedProjectViews.length - 1]?.textContent).toBe('ProjectsView: true');
    expect(projectsViewRenderMock).toHaveBeenLastCalledWith(expect.objectContaining({ isActive: true }));
  });

  it('keeps one transcript workbench instance while switching tabs around projects mode', () => {
    const runtimeState = {
      mode: 'projects' as 'projects' | 'batch' | 'live',
      setMode: vi.fn(),
    };
    mockUseTranscriptRuntimeStore.mockImplementation((selector: any) => selector(runtimeState));

    const { rerender } = render(<App />);

    expect(screen.getByTestId('transcript-workbench')).toBeDefined();
    expect(transcriptWorkbenchMountMock).toHaveBeenCalledTimes(1);

    runtimeState.mode = 'batch';
    rerender(<App />);

    expect(screen.getByTestId('transcript-workbench')).toBeDefined();
    expect(transcriptWorkbenchMountMock).toHaveBeenCalledTimes(1);
    expect(transcriptWorkbenchUnmountMock).not.toHaveBeenCalled();

    runtimeState.mode = 'projects';
    rerender(<App />);

    expect(screen.getByTestId('transcript-workbench')).toBeDefined();
    expect(transcriptWorkbenchMountMock).toHaveBeenCalledTimes(1);
    expect(transcriptWorkbenchUnmountMock).not.toHaveBeenCalled();
  });

  it('keeps the shared editor mounted but hidden when projects mode has no active transcript', () => {
    mockUseTranscriptRuntimeStore.mockImplementation((selector: any) => selector({
      mode: 'projects',
      setMode: vi.fn(),
    }));

    const { container } = render(<App />);

    expect(screen.getByTestId('transcript-workbench')).toBeDefined();
    expect(container.querySelector('.persistent-transcript-host')?.classList.contains('is-hidden')).toBe(true);
    expect(screen.getByTestId('projects-view').textContent).toBe('ProjectsView: true');
  });

  it('closes the shared transcript session and hides the projects detail host', async () => {
    mockUseTranscriptRuntimeStore.mockImplementation((selector: any) => selector({
      mode: 'projects',
      setMode: vi.fn(),
    }));
    useTranscriptSessionStore.getState().openSession({
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true }],
      sourceHistoryId: 'hist-1',
      title: 'History Item',
      icon: null,
    });
    useTranscriptPlaybackStore.getState().openSession('mock-audio-url');

    const { container } = render(<App />);

    expect(container.querySelector('.persistent-transcript-host')?.classList.contains('is-hidden')).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: 'Close Transcript' }));

    await waitFor(() => {
      expect(container.querySelector('.persistent-transcript-host')?.classList.contains('is-hidden')).toBe(true);
    });
    expect(useTranscriptSessionStore.getState().sourceHistoryId).toBeNull();
    expect(useTranscriptSessionStore.getState().segments).toHaveLength(0);
    expect(useTranscriptPlaybackStore.getState().audioUrl).toBeNull();
  });
});
