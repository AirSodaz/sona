import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
vi.mock('../components/GlobalDialog', () => ({ GlobalDialog: () => <div>GlobalDialog</div> }));
vi.mock('../components/ErrorDialog', () => ({ ErrorDialog: () => <div>ErrorDialog</div> }));
vi.mock('../components/FirstRunGuide', () => ({ FirstRunGuide: () => <div>FirstRunGuide</div> }));
vi.mock('../components/NotificationCenter', () => ({ NotificationCenter: () => null }));
vi.mock('../components/OnboardingReminderBanner', () => ({ OnboardingReminderBanner: () => <div>OnboardingReminderBanner</div> }));
vi.mock('../components/Icons', () => ({
  SettingsIcon: () => <span>SettingsIcon</span>,
}));

vi.mock('../components/Settings', () => ({
  Settings: ({ isOpen, initialTab, onOpenDiagnostics }: any) => (
    isOpen ? (
      <div>
        <div>Settings Tab: {initialTab}</div>
        <button type="button" onClick={onOpenDiagnostics}>Open Diagnostics</button>
      </div>
    ) : null
  ),
}));

vi.mock('../components/DiagnosticsModal', () => ({
  DiagnosticsModal: ({ isOpen, onOpenSettingsTab }: any) => (
    isOpen ? (
      <div>
        <div>Diagnostics Modal</div>
        <button type="button" onClick={() => onOpenSettingsTab('models')}>Open Model Settings</button>
      </div>
    ) : null
  ),
}));

vi.mock('../components/RecoveryCenterModal', () => ({
  RecoveryCenterModal: () => null,
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

describe('App diagnostics flow', () => {
  it('closes settings when opening diagnostics and reopens settings on the requested tab', async () => {
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

    fireEvent.click(screen.getByRole('button', { name: 'header.settings' }));
    expect(await screen.findByText('Settings Tab: general')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Open Diagnostics' }));
    await waitFor(() => {
      expect(screen.queryByText('Settings Tab: general')).toBeNull();
    });
    expect(await screen.findByText('Diagnostics Modal')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Open Model Settings' }));
    await waitFor(() => {
      expect(screen.queryByText('Diagnostics Modal')).toBeNull();
    });
    expect(await screen.findByText('Settings Tab: models')).toBeDefined();
  });
});
