import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { Settings } from '../Settings';
import { modelService } from '../../services/modelService';

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
            streamingModelPath: '/test/streaming',
            offlineModelPath: '/test/offline',
            enableITN: true,
            appLanguage: 'auto'
        },
        setConfig: mockSetConfig
    })
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [
        { id: 'test-model', name: 'Test Model', language: 'en', type: 'streaming', size: '100MB', description: 'Test', engine: 'onnx' }
    ],
    ITN_MODELS: [
        { id: 'itn-zh-number', name: 'Chinese Number ITN', description: 'Test ITN', filename: 'itn_zh_number.fst' }
    ],
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false),
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn(),
        deleteModel: vi.fn(),
        getITNModelPath: vi.fn(),
        downloadITNModel: vi.fn(),
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

    it('renders with vertical layout structure', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Wait for async checks on mount to settle
        await waitFor(() => {
             expect(modelService.isModelInstalled).toHaveBeenCalled();
        });

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

    it('renders accessible model list buttons', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Wait for mount effects
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.model_hub'));

        // Check for accessible buttons
        // The mock returns keys for translations, so we expect "common.download Test Model"
        expect(screen.getByLabelText('common.download Test Model')).toBeDefined();
    });

    it('switches tabs correctly', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Switch to Model Hub
        fireEvent.click(screen.getByText('settings.model_hub'));
        expect(screen.getByText('Test Model')).toBeDefined();
        expect(screen.queryByText('settings.language')).toBeNull(); // Should disappear

        // Switch to Local Path
        fireEvent.click(screen.getByText('settings.local_path'));
        expect(screen.getByDisplayValue('/test/streaming')).toBeDefined();
    });

    it('closes when close button is clicked', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Find close button (X icon usually has aria-label="Close")
        const closeBtn = screen.getByLabelText('common.close');
        fireEvent.click(closeBtn);

        expect(onClose).toHaveBeenCalled();
    });

    it('saves configuration and closes', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Click Save
        fireEvent.click(screen.getByText('settings.save_button'));

        expect(mockSetConfig).toHaveBeenCalledWith({
            streamingModelPath: '/test/streaming',
            offlineModelPath: '/test/offline',
            punctuationModelPath: '',
            enableITN: true,
            enabledITNModels: ['itn-zh-number'],
            itnRulesOrder: ['itn-zh-number'],
            appLanguage: 'auto',
            theme: 'auto',
            font: 'system',
            vadModelPath: '',
            vadBufferSize: 5
        });
        expect(onClose).toHaveBeenCalled();
    });

    it('renders accessible ITN toggle switches', async () => {
        vi.mocked(modelService.isITNModelInstalled).mockResolvedValue(true);
        render(<Settings isOpen={true} onClose={onClose} />);

        // Wait for checks
        await waitFor(() => expect(modelService.isITNModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.local_path'));

        // Should find the switch because we mocked isITNModelInstalled to true
        // and useTranslation mock returns the key
        const toggle = await screen.findByRole('switch');
        expect(toggle).toBeDefined();
        expect(toggle.getAttribute('aria-label')).toBe('settings.toggle_model');
    });

    it('implements ARIA tabs pattern', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

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
