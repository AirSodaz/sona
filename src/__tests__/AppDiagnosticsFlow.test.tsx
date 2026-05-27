import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import App from '../App';
import { PanelModal } from '../components/PanelModal';

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
vi.mock('../components/transcript/TranscriptWorkbench', () => ({ TranscriptWorkbench: () => <div>TranscriptWorkbench</div> }));
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
      <div className="settings-overlay" data-testid="settings-overlay">
        <div data-testid="settings-modal">
          <div>Settings Tab: {initialTab}</div>
          <button type="button" onClick={onOpenDiagnostics}>Open Diagnostics</button>
        </div>
      </div>
    ) : null
  ),
}));

vi.mock('../components/settings/settingsLoaders', () => ({
  preloadAllSettingsTabs: vi.fn().mockResolvedValue(undefined),
  preloadSettingsTab: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../components/DiagnosticsModal', () => ({
  DiagnosticsModal: ({ isOpen, onOpenSettingsTab, origin, onBack }: any) => (
    isOpen ? (
      <PanelModal
        isOpen={isOpen}
        onClose={vi.fn()}
        ariaLabel="Diagnostics Modal"
        origin={origin}
        onBack={onBack}
        backLabel="Back to Settings"
        size="settings"
        title="Diagnostics Modal"
      >
        <div data-testid="diagnostics-modal">
          <div>Diagnostics Modal</div>
          <div>Diagnostics Origin: {origin}</div>
          <button type="button" onClick={() => onOpenSettingsTab('models')}>Open Model Settings</button>
        </div>
      </PanelModal>
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

describe('App diagnostics flow', () => {
  it('keeps settings open under diagnostics and reopens the requested tab flow from settings origin', async () => {
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

    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: 'header.settings' }));
    expect(await screen.findByText('Settings Tab: general')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Open Diagnostics' }));
    await waitFor(() => {
      expect(screen.getByTestId('settings-modal')).toBeDefined();
    });
    expect(await screen.findByRole('dialog', { name: 'Diagnostics Modal' })).toBeDefined();
    expect(screen.getByText('Diagnostics Origin: settings')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Back to Settings' })).toBeDefined();
    expect(document.querySelectorAll('.settings-overlay')).toHaveLength(1);
    expect(document.querySelectorAll('.panel-modal-overlay.panel-modal-origin-settings')).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: 'Open Model Settings' }));
    await waitFor(() => {
      expect(screen.queryByText('Diagnostics Modal')).toBeNull();
    });
    expect(await screen.findByText('Settings Tab: models')).toBeDefined();
  });

  it('returns to the existing settings modal when back is pressed', async () => {
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

    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: 'header.settings' }));
    expect(await screen.findByText('Settings Tab: general')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Open Diagnostics' }));
    expect(await screen.findByRole('dialog', { name: 'Diagnostics Modal' })).toBeDefined();
    expect(document.querySelectorAll('.settings-overlay')).toHaveLength(1);
    expect(document.querySelectorAll('.panel-modal-overlay.panel-modal-origin-settings')).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: 'Back to Settings' }));

    await waitFor(() => {
      expect(screen.queryByText('Diagnostics Modal')).toBeNull();
    });
    expect(screen.getByText('Settings Tab: general')).toBeDefined();
  });
});
