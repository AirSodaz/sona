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

const mockSetConfig = vi.fn();
vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => selector({
        config: {
            recognitionModelPath: '/test/recognition',
            punctuationModelPath: '/test/punctuation',
            enableITN: true,
            appLanguage: 'auto'
        },
        setConfig: mockSetConfig
    })
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [
        { id: 'test-model', name: 'Test Model', language: 'en', type: 'recognition', size: '100MB', description: 'Test', engine: 'onnx' }
    ],
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn(),
        deleteModel: vi.fn(),
    }
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
    ask: vi.fn(),
    message: vi.fn(),
}));

describe('Settings', () => {
    const onClose = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders with vertical layout structure', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Check for Sidebar items (buttons)
        // Note: settings.general appears in both sidebar button and header
        expect(screen.getAllByText('settings.general').length).toBeGreaterThanOrEqual(1);

        // Check for specific sidebar buttons if possible or just existence
        expect(screen.getByText('settings.model_hub')).toBeDefined();
        expect(screen.getByText('settings.local_path')).toBeDefined();

        // Check initial active tab content
        expect(screen.getByLabelText('settings.language')).toBeDefined();
        expect(screen.getByLabelText('settings.theme')).toBeDefined();
    });

    it('renders accessible model list buttons', () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        fireEvent.click(screen.getByText('settings.model_hub'));

        // Check for accessible buttons
        // The mock returns keys for translations, so we expect "common.download Test Model"
        expect(screen.getByLabelText('common.download Test Model')).toBeDefined();
    });

    it('switches tabs correctly', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Switch to Model Hub
        fireEvent.click(screen.getByText('settings.model_hub'));
        expect(screen.getByText('Test Model')).toBeDefined();
        expect(screen.queryByText('settings.language')).toBeNull(); // Should disappear

        // Switch to Local Path
        fireEvent.click(screen.getByText('settings.local_path'));
        expect(screen.getByDisplayValue('/test/recognition')).toBeDefined();
    });

    it('closes when close button is clicked', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Find close button (X icon usually has aria-label="Close")
        const closeBtn = screen.getByLabelText('common.close');
        fireEvent.click(closeBtn);

        expect(onClose).toHaveBeenCalled();
    });

    it('saves configuration and closes', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Click Save
        fireEvent.click(screen.getByText('settings.save_button'));

        expect(mockSetConfig).toHaveBeenCalledWith({
            recognitionModelPath: '/test/recognition',
            punctuationModelPath: '/test/punctuation',
            vadModelPath: '',
            enableITN: true,
            appLanguage: 'auto',
            theme: 'auto',
            font: 'system'
        });
        expect(onClose).toHaveBeenCalled();
    });

    it('renders ITN toggle switch', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        fireEvent.click(screen.getByText('settings.local_path'));

        // Should find the switch for enable ITN
        const toggle = await screen.findByRole('switch');
        expect(toggle).toBeDefined();
        expect(toggle.getAttribute('aria-checked')).toBe('true');
        expect(toggle.getAttribute('aria-label')).toBe('settings.itn_title');
        expect(toggle.getAttribute('data-tooltip')).toBe('settings.itn_title');
    });

    it('implements ARIA tabs pattern', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Check for tablist
        const tablist = screen.getByRole('tablist');
        expect(tablist).toBeDefined();
        expect(tablist.getAttribute('aria-orientation')).toBe('vertical');

        // Check for tabs
        const tabs = screen.getAllByRole('tab');
        expect(tabs).toHaveLength(3);

        // Check "General" tab (active by default)
        const generalTab = tabs[0];
        expect(generalTab.getAttribute('aria-selected')).toBe('true');
        expect(generalTab.getAttribute('aria-controls')).toBe('settings-panel-general');

        // Check panel
        const panel = screen.getByRole('tabpanel');
        expect(panel).toBeDefined();
        expect(panel.id).toBe('settings-panel-general');
        expect(panel.getAttribute('aria-labelledby')).toBe(generalTab.id);

        // Switch to "Models"
        const modelTab = tabs[1];
        fireEvent.click(modelTab);

        expect(modelTab.getAttribute('aria-selected')).toBe('true');
        expect(generalTab.getAttribute('aria-selected')).toBe('false');

        const newPanel = screen.getByRole('tabpanel');
        expect(newPanel.id).toBe('settings-panel-models');
        expect(newPanel.getAttribute('aria-labelledby')).toBe(modelTab.id);
    });
});
