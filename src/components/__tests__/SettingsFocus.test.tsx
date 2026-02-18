import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Settings } from '../Settings';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() },
    }),
}));

const setActiveTabMock = vi.fn();

vi.mock('../../hooks/useSettingsLogic', () => ({
    useSettingsLogic: () => ({
        activeTab: 'general',
        setActiveTab: setActiveTabMock,
        appLanguage: 'auto',
        theme: 'auto',
        font: 'system',
        streamingModelPath: '',
        offlineModelPath: '',
        punctuationModelPath: '',
        vadModelPath: '',
        vadBufferSize: 5,
        itnRulesOrder: [],
        enabledITNModels: new Set(),
        installedITNModels: new Set(),
        downloadingId: null,
        deletingId: null,
        progress: 0,
        statusMessage: '',
        installedModels: new Set(),
        handleSave: vi.fn(),
        handleBrowse: vi.fn(),
        handleDownload: vi.fn(),
        handleDownloadITN: vi.fn(),
        handleCancelDownload: vi.fn(),
        handleLoad: vi.fn(),
        handleDelete: vi.fn(),
        isModelSelected: vi.fn(),
        maxConcurrent: 2,
        setMaxConcurrent: vi.fn(),
    })
}));

// Mock sub-components to avoid rendering complexity
vi.mock('../settings/SettingsGeneralTab', () => ({
    SettingsGeneralTab: () => <button>General Tab Input</button>
}));
vi.mock('../settings/SettingsModelsTab', () => ({
    SettingsModelsTab: () => <div>Models Tab</div>
}));
vi.mock('../settings/SettingsLocalTab', () => ({
    SettingsLocalTab: () => <div>Local Tab</div>
}));
vi.mock('../Icons', () => ({
    GeneralIcon: () => <span>Icon</span>,
    ModelIcon: () => <span>Icon</span>,
    LocalIcon: () => <span>Icon</span>,
    KeyboardIcon: () => <span>Icon</span>,
    InfoIcon: () => <span>Icon</span>,
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

        const tablist = screen.getByRole('tablist');
        const tabs = screen.getAllByRole('tab');
        const generalTab = tabs[0];

        // Focus the first tab
        generalTab.focus();

        // Arrow Down -> Should switch to 'models'
        fireEvent.keyDown(tablist, { key: 'ArrowDown' });
        expect(setActiveTabMock).toHaveBeenCalledWith('models');

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
});
