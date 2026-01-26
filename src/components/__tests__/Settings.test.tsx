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
        expect(screen.getByDisplayValue('/test/streaming')).toBeDefined();
    });

    it('closes when close button is clicked', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Find close button (X icon usually has aria-label="Close")
        const closeBtn = screen.getByLabelText('Close');
        fireEvent.click(closeBtn);

        expect(onClose).toHaveBeenCalled();
    });

    it('saves configuration and closes', () => {
        render(<Settings isOpen={true} onClose={onClose} />);

        // Click Save
        fireEvent.click(screen.getByText('settings.save_button'));

        expect(mockSetConfig).toHaveBeenCalledWith({
            streamingModelPath: '/test/streaming',
            offlineModelPath: '/test/offline',
            enableITN: true,
            appLanguage: 'auto',
            theme: 'auto'
        });
        expect(onClose).toHaveBeenCalled();
    });
});
