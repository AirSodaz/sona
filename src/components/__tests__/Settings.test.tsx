import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { Settings } from '../Settings';
import { modelService } from '../../services/modelService';
import { useDialogStore } from '../../stores/dialogStore';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() },
    }),
}));

const mockSetConfig = vi.fn();
const mockState = {
    config: {
        streamingModelPath: '/test/streaming',
        offlineModelPath: '/test/offline',
        enableITN: true,
        enabledITNModels: ['itn-zh-number'],
        itnRulesOrder: ['itn-zh-number'],
        appLanguage: 'auto'
    },
    setConfig: mockSetConfig
};

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: (state: any) => any) => selector(mockState)
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [
        { id: 'test-model', name: 'Test Model', language: 'en', type: 'streaming', size: '100MB', description: 'Test', engine: 'onnx', filename: 'test-model' }
    ],
    ITN_MODELS: [
        { id: 'itn-zh-number', name: 'Chinese Number ITN', description: 'Test ITN', filename: 'itn_zh_number.fst' }
    ],
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false),
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn().mockResolvedValue('/path/to/model'),
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
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());
        expect(screen.getAllByText('settings.general').length).toBeGreaterThanOrEqual(1);
    });

    it('downloads a model successfully', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.model_hub'));

        // Find download button for 'Test Model'
        // Since we mock translation, label is "common.download Test Model"
        const downloadBtn = screen.getByLabelText('common.download Test Model');

        // Simulate download
        vi.mocked(modelService.downloadModel).mockImplementation(async (_id, onProgress) => {
            if (onProgress) {
                onProgress(50, 'Downloading...');
                onProgress(100, 'Done');
            }
            return '/path/to/downloaded/model';
        });

        fireEvent.click(downloadBtn);

        // Check for progress bar
        await waitFor(() => {
            expect(screen.getByRole('progressbar')).toBeDefined();
        });

        // Wait for completion (modelService.downloadModel resolves)
        await waitFor(() => {
            expect(modelService.downloadModel).toHaveBeenCalledWith('test-model', expect.any(Function), expect.any(AbortSignal));
        });
    });

    it('deletes a model', async () => {
        // Setup: Model is installed
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);
        vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(true);

        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.model_hub'));

        // Find delete button
        const deleteBtn = await screen.findByLabelText('common.delete Test Model');
        fireEvent.click(deleteBtn);

        // Verify confirm was called
        expect(useDialogStore.getState().confirm).toHaveBeenCalled();

        // Verify deleteModel was called
        await waitFor(() => {
            expect(modelService.deleteModel).toHaveBeenCalledWith('test-model');
        });

        // Verify list refreshes
        expect(modelService.isModelInstalled).toHaveBeenCalledTimes(2); // Initial + after delete
    });

    it('loads a model path when Load is clicked', async () => {
        // Setup: Model is installed
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);

        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.model_hub'));

        const loadBtn = await screen.findByLabelText('settings.load Test Model');
        fireEvent.click(loadBtn);

        expect(modelService.getModelPath).toHaveBeenCalledWith('test-model');

        // Check if path was set in state (switch to Local Path tab to verify)
        fireEvent.click(screen.getByText('settings.local_path'));
        await waitFor(() => {
            expect(screen.getByDisplayValue('/path/to/model')).toBeDefined();
        });
    });

    it('saves configuration', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.save_button'));

        expect(mockSetConfig).toHaveBeenCalled();
        expect(onClose).toHaveBeenCalled();
    });
});
