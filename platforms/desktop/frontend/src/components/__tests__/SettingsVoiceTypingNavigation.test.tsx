import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { Settings } from '../Settings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: vi.fn() },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../hooks/useFocusTrap', () => ({
  useFocusTrap: vi.fn(),
}));

vi.mock('../../hooks/useModelManager', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../hooks/useModelManager')>();
  return {
    ...actual,
    useModelManager: () => ({
      deletingId: null,
      downloads: {},
      installedModels: new Set(),
      handleDownload: vi.fn(),
      handleCancelDownload: vi.fn(),
      handleLoad: vi.fn(),
      handleDelete: vi.fn(),
      restoreDefaultModelSettings: vi.fn(),
    }),
  };
});

vi.mock('../settings/SettingsGeneralTab', () => ({
  SettingsGeneralTab: () => <div>General Tab</div>,
}));

vi.mock('../settings/SettingsSubtitleTab', () => ({
  SettingsSubtitleTab: () => <div>Subtitle Tab</div>,
}));

vi.mock('../settings/SettingsLLMServiceTab', () => ({
  SettingsLLMServiceTab: () => <div>LLM Tab</div>,
}));

vi.mock('../settings/SettingsShortcutsTab', () => ({
  SettingsShortcutsTab: () => <div>Shortcuts Tab</div>,
}));

vi.mock('../settings/SettingsAboutTab', () => ({
  SettingsAboutTab: () => <div>About Tab</div>,
}));

vi.mock('../settings/SettingsVocabularyTab', () => ({
  SettingsVocabularyTab: () => <div>Vocabulary Tab</div>,
}));

vi.mock('../settings/SettingsAutomationTab', () => ({
  SettingsAutomationTab: () => <div>Automation Tab</div>,
}));

vi.mock('../settings/SettingsModelsTab', () => ({
  SettingsModelsTab: () => <div>Models Tab</div>,
}));

vi.mock('../settings/SettingsMicrophoneTab', () => ({
  SettingsMicrophoneTab: () => <div>Microphone Tab</div>,
}));

describe('Settings voice typing navigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
      },
    });
  });

  it('maps the legacy voice typing initial tab to the migrated banner', async () => {
    const onClose = vi.fn();
    render(<Settings isOpen={true} onClose={onClose} initialTab="voice_typing" />);

    expect((await screen.findAllByText('tools.voice_typing')).length).toBeGreaterThan(0);
    expect(await screen.findByText('tools.migrated_notice')).toBeDefined();
    expect(screen.queryByRole('tab', { name: 'settings.voice_typing' })).toBeNull();
  });
});
