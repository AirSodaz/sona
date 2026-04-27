import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
            isModelSelected: vi.fn(),
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

vi.mock('../settings/SettingsVoiceTypingTab', async () => {
    const { useSettingsNavigation } = await import('../settings/SettingsNavigationContext');

    return {
        SettingsVoiceTypingTab: () => {
            const { navigateToTab } = useSettingsNavigation();

            return (
                <div>
                    <div>Voice Typing Tab</div>
                    <button type="button" onClick={() => navigateToTab('models')}>
                        Open Model Hub
                    </button>
                    <button type="button" onClick={() => navigateToTab('microphone')}>
                        Open Input Device
                    </button>
                </div>
            );
        },
    };
});

describe('Settings voice typing navigation', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        useConfigStore.setState({
            config: {
                ...DEFAULT_CONFIG,
            },
        });
    });

    it('switches from the voice typing page to model hub and focuses the target tab button', async () => {
        render(<Settings isOpen={true} onClose={vi.fn()} initialTab="voice_typing" />);

        fireEvent.click(await screen.findByRole('button', { name: 'Open Model Hub' }));

        const modelsTab = await screen.findByRole('tab', { name: 'settings.model_hub' });
        await waitFor(() => {
            expect(screen.getByText('Models Tab')).toBeDefined();
            expect(document.activeElement).toBe(modelsTab);
        });
    });

    it('switches from the voice typing page to input device and focuses the target tab button', async () => {
        render(<Settings isOpen={true} onClose={vi.fn()} initialTab="voice_typing" />);

        fireEvent.click(await screen.findByRole('button', { name: 'Open Input Device' }));

        const microphoneTab = await screen.findByRole('tab', { name: 'settings.input_device' });
        await waitFor(() => {
            expect(screen.getByText('Microphone Tab')).toBeDefined();
            expect(document.activeElement).toBe(microphoneTab);
        });
    });
});
