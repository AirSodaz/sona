import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import App from '../App';

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
vi.mock('../components/ExportButton', () => ({ ExportButton: () => <div>ExportButton</div> }));
vi.mock('../components/TranslateButton', () => ({ TranslateButton: () => <div>TranslateButton</div> }));
vi.mock('../components/PolishButton', () => ({ PolishButton: () => <div>PolishButton</div> }));
vi.mock('../components/BatchImport', () => ({ BatchImport: () => <div>BatchImport</div> }));
vi.mock('../components/LiveRecord', () => ({ LiveRecord: () => <div>LiveRecord</div> }));
vi.mock('../components/ProjectsView', () => ({ ProjectsView: () => <div>ProjectsView</div> }));
vi.mock('../components/Settings', () => ({ Settings: () => null }));
vi.mock('../components/DiagnosticsModal', () => ({ DiagnosticsModal: () => null }));
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => null }));
vi.mock('../components/UpdateNotification', () => ({ UpdateNotification: () => <div>UpdateNotification</div> }));
vi.mock('../components/Icons', () => ({
  SettingsIcon: () => <span>SettingsIcon</span>,
}));

vi.mock('../components/RecoveryStartupBanner', () => ({
  RecoveryStartupBanner: ({ onOpenRecoveryCenter }: any) => (
    <button type="button" onClick={onOpenRecoveryCenter}>Open Recovery Banner</button>
  ),
}));

vi.mock('../components/RecoveryCenterModal', () => ({
  RecoveryCenterModal: ({ isOpen }: any) => (
    isOpen ? <div>Recovery Center Modal</div> : null
  ),
}));

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

const mockUseTranscriptStore = vi.fn();
const mockUseProjectStore = vi.fn();
const mockUseOnboardingStore = vi.fn();

vi.mock('../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => mockUseTranscriptStore(selector),
}));

vi.mock('../stores/projectStore', () => ({
  useProjectStore: (selector: any) => mockUseProjectStore(selector),
}));

vi.mock('../stores/onboardingStore', () => ({
  useOnboardingStore: (selector: any) => mockUseOnboardingStore(selector),
}));

describe('App recovery flow', () => {
  it('opens the recovery center from the startup banner entry', () => {
    mockUseTranscriptStore.mockImplementation((selector: any) => selector({
      mode: 'live',
      clearSegments: vi.fn(),
      setMode: vi.fn(),
    }));
    mockUseProjectStore.mockImplementation((selector: any) => selector({
      activeProjectId: null,
      projects: [],
    }));
    mockUseOnboardingStore.mockImplementation((selector: any) => selector({
      reopen: vi.fn(),
    }));

    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: 'Open Recovery Banner' }));

    expect(screen.getByText('Recovery Center Modal')).toBeDefined();
  });
});
