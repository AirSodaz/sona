import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Settings } from '../Settings';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';

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

    it('maps the legacy voice typing initial tab to the combined subtitle page', async () => {
        render(<Settings isOpen={true} onClose={vi.fn()} initialTab="voice_typing" />);

        const subtitleTab = await screen.findByRole('tab', { name: 'settings.subtitle_voice_typing_title' });
        expect(subtitleTab.getAttribute('aria-selected')).toBe('true');
        expect(await screen.findByText('Subtitle Tab')).toBeDefined();
        expect(screen.queryByRole('tab', { name: 'settings.voice_typing' })).toBeNull();
    });
});
