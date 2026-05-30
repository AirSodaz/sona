import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { Settings } from '../Settings';
import type { SettingsTab } from '../../hooks/useSettingsLogic';

// Mock dependencies
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

const setActiveTabMock = vi.fn();
const modelPanePropsMock = vi.hoisted(() => vi.fn());
let mockActiveTab: SettingsTab = 'general';

vi.mock('../../hooks/useSettingsLogic', () => ({
    useSettingsLogic: () => ({
        activeTab: mockActiveTab,
        setActiveTab: setActiveTabMock,
        config: {
            appLanguage: 'auto',
            language: 'auto',
        },
        updateConfig: vi.fn(),
    })
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
        })
    };
});

vi.mock('../../hooks/useLlmConfig', () => ({
    useLlmConfig: () => ({
        changeLlmServiceType: vi.fn(),
    })
}));

// Mock sub-components to avoid rendering complexity
vi.mock('../settings/SettingsGeneralTab', () => ({
    SettingsGeneralTab: () => <button>General Tab Input</button>
}));
vi.mock('../settings/SettingsDashboardTab', () => ({
    SettingsDashboardTab: () => <div>Dashboard Tab</div>
}));
vi.mock('../settings/SettingsMicrophoneTab', () => ({
    SettingsMicrophoneTab: () => <div>Microphone Tab</div>
}));
vi.mock('../settings/SettingsSubtitleTab', () => ({
    SettingsSubtitleTab: () => <div>Subtitle Tab</div>
}));
vi.mock('../settings/SettingsModelsTab', () => ({
    SettingsModelsTab: () => <div>Models Tab</div>
}));
vi.mock('../settings/SettingsModelsPane', () => ({
    SettingsModelsPane: (props: any) => {
        modelPanePropsMock(props);
        return <div>Models Tab</div>;
    },
}));
vi.mock('../settings/SettingsVoiceTypingTab', () => ({
    SettingsVoiceTypingTab: () => <div>Voice Typing Tab</div>
}));
vi.mock('../settings/SettingsVocabularyTab', () => ({
    SettingsVocabularyTab: () => <div>Vocabulary Tab</div>
}));
vi.mock('../settings/SettingsAutomationTab', () => ({
    SettingsAutomationTab: () => <div>Automation Tab</div>
}));
vi.mock('../settings/SettingsLLMServiceTab', () => ({
    SettingsLLMServiceTab: () => <div>LLM Service Tab</div>
}));
vi.mock('../settings/SettingsShortcutsTab', () => ({
    SettingsShortcutsTab: () => <div>Shortcuts Tab</div>
}));
vi.mock('../settings/SettingsAboutTab', () => ({
    SettingsAboutTab: () => <div>About Tab</div>
}));
vi.mock('../Icons', () => ({
    GeneralIcon: () => <span>Icon</span>,
    MicIcon: () => <span>Icon</span>,
    SubtitleIcon: () => <span>Icon</span>,
    ModelIcon: () => <span>Icon</span>,
    AutomationIcon: () => <span>Icon</span>,
    KeyboardIcon: () => <span>Icon</span>,
    InfoIcon: () => <span>Icon</span>,
    RobotIcon: () => <span>Icon</span>,
    BookIcon: () => <span>Icon</span>,
    ChevronDownIcon: () => <span>Icon</span>,
    XIcon: () => <span>X</span>
}));

// Mock dialog store
vi.mock('../stores/dialogStore', () => ({
    useDialogStore: {
        getState: () => ({ isOpen: false })
    }
}));

describe('Settings Focus Trap & Navigation', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockActiveTab = 'general';
        modelPanePropsMock.mockClear();
    });

    it('traps focus inside the modal', async () => {
        const onClose = vi.fn();
        render(<Settings isOpen={true} onClose={onClose} />);

        const modal = screen.getByRole('dialog');

        const focusableSelector = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])';
        const focusableElements = modal.querySelectorAll(focusableSelector);

        expect(focusableElements.length).toBeGreaterThan(0);

        const firstElement = focusableElements[0] as HTMLElement;
        const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

        // Focus the last element
        lastElement.focus();
        expect(document.activeElement).toBe(lastElement);

        // Press Tab (should cycle to first)
        fireEvent.keyDown(window, { key: 'Tab', shiftKey: false });
        expect(document.activeElement).toBe(firstElement);

        // Focus the first element
        firstElement.focus();
        expect(document.activeElement).toBe(firstElement);

        // Press Shift+Tab (should cycle to last)
        fireEvent.keyDown(window, { key: 'Tab', shiftKey: true });
        expect(document.activeElement).toBe(lastElement);
    });

    it('navigates tabs with arrow keys', () => {
        const onClose = vi.fn();
        render(<Settings isOpen={true} onClose={onClose} />);

        expect(screen.getByText('settings.vocabulary')).toBeDefined();
        expect(screen.queryByText('settings.context_title')).toBeNull();
        expect(screen.queryByText('settings.local_path')).toBeNull();

        const tablist = screen.getByRole('tablist');
        const tabs = screen.getAllByRole('tab');
        const generalTab = tabs[0];

        // Focus the first tab
        generalTab.focus();

        // Arrow Down -> Should switch to 'dashboard'
        fireEvent.keyDown(tablist, { key: 'ArrowDown' });
        expect(setActiveTabMock).toHaveBeenCalledWith('dashboard');

        // Reset mock
        setActiveTabMock.mockClear();

        // Arrow Up -> Should switch to 'about' (loops around from general)
        fireEvent.keyDown(tablist, { key: 'ArrowUp' });
        expect(setActiveTabMock).toHaveBeenCalledWith('about');

        // Reset mock
        setActiveTabMock.mockClear();

        // End -> Should switch to 'about' (last)
        fireEvent.keyDown(tablist, { key: 'End' });
        expect(setActiveTabMock).toHaveBeenCalledWith('about');

        // Reset mock
        setActiveTabMock.mockClear();

        // Home -> Should switch to 'general' (first)
        fireEvent.keyDown(tablist, { key: 'Home' });
        expect(setActiveTabMock).toHaveBeenCalledWith('general');
    });

    it('renders the voice typing tab between subtitle settings and model hub', () => {
        const onClose = vi.fn();
        render(<Settings isOpen={true} onClose={onClose} />);

        expect(screen.getByText('settings.voice_typing')).toBeDefined();

        const tabLabels = screen
            .getAllByRole('tab')
            .map((tab) => tab.textContent?.trim()?.replace(/^Icon/, ''));

        expect(tabLabels).toEqual([
            'settings.general',
            'settings.dashboard.title',
            'settings.input_device',
            'live.subtitle_settings',
            'settings.voice_typing',
            'settings.model_hub',
            'settings.vocabulary',
            'settings.automation',
            'settings.api_server.title',
            'settings.llm.title',
            'shortcuts.title',
            'settings.about',
        ]);
    });

    it('renders the dedicated voice typing panel when that tab is active', async () => {
        mockActiveTab = 'voice_typing';
        const onClose = vi.fn();

        render(<Settings isOpen={true} onClose={onClose} />);

        expect(await screen.findByText('Voice Typing Tab')).toBeDefined();
        expect(screen.getByText('General Tab Input').closest('[hidden]')).not.toBeNull();
    });

    it('navigates tabs with ctrl+tab without the removed local tab', () => {
        const onClose = vi.fn();
        render(<Settings isOpen={true} onClose={onClose} />);

        fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
        expect(setActiveTabMock).toHaveBeenCalledWith('dashboard');

        setActiveTabMock.mockClear();

        fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true, shiftKey: true });
        expect(setActiveTabMock).toHaveBeenCalledWith('about');
    });

    it('keeps prewarmed inactive panes closed when the settings dialog opens', async () => {
        const onClose = vi.fn();
        const { rerender } = render(<Settings isOpen={false} prewarm onClose={onClose} />);

        await waitFor(() => {
            expect(modelPanePropsMock).toHaveBeenCalled();
        });

        modelPanePropsMock.mockClear();
        rerender(<Settings isOpen={true} onClose={onClose} />);

        await waitFor(() => {
            expect(screen.getByRole('dialog')).toBeDefined();
        });

        const inactivePropsAfterOpen = modelPanePropsMock.mock.calls.map(([props]) => props);
        expect(inactivePropsAfterOpen.every((props) => props.isOpen === false && props.isActive === false)).toBe(true);
    });

    it('resets scroll position to 0 when active tab changes', async () => {
        const onClose = vi.fn();
        mockActiveTab = 'general';
        const { rerender, container } = render(<Settings isOpen={true} onClose={onClose} />);

        const scrollContainer = container.querySelector('.settings-content-scroll') as HTMLElement;
        expect(scrollContainer).not.toBeNull();

        // Mock scrollTop and set it to a non-zero value
        scrollContainer.scrollTop = 100;
        expect(scrollContainer.scrollTop).toBe(100);

        // Change active tab and rerender within act
        await act(async () => {
            mockActiveTab = 'dashboard';
            rerender(<Settings isOpen={true} onClose={onClose} />);
        });

        // Scroll position should be reset to 0
        expect(scrollContainer.scrollTop).toBe(0);
    });
});
