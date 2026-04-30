import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import App from '../App';

const settingsModuleLoaded = vi.hoisted(() => vi.fn());

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
vi.mock('../components/TranscriptWorkbench', () => ({ TranscriptWorkbench: () => <div>TranscriptWorkbench</div> }));
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/DiagnosticsModal', () => ({ DiagnosticsModal: () => null }));
vi.mock('../components/RecoveryCenterModal', () => ({ RecoveryCenterModal: () => null }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/NotificationCenter', () => ({ NotificationCenter: () => null }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => null }));
vi.mock('../components/Icons', () => ({
  AutomationIcon: () => <span>AutomationIcon</span>,
  SettingsIcon: () => <span>SettingsIcon</span>,
}));

vi.mock('../components/Settings', () => {
  settingsModuleLoaded();

  return {
    Settings: ({ isOpen, initialTab }: any) => (
      isOpen ? <div>Settings Tab: {initialTab}</div> : null
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
    vi.stubGlobal('requestIdleCallback', vi.fn(() => 1));
    vi.stubGlobal('cancelIdleCallback', vi.fn());
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
    expect(settingsModuleLoaded).not.toHaveBeenCalled();

    fireEvent.pointerEnter(settingsButton);

    await waitFor(() => {
      expect(settingsModuleLoaded).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(settingsButton);

    expect(await screen.findByText('Settings Tab: general')).toBeDefined();
    expect(screen.queryByRole('dialog', { name: 'common.loading' })).toBeNull();
  });
});
