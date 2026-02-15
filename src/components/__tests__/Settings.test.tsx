import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { Settings } from '../Settings';
import { modelService } from '../../services/modelService';
import { useDialogStore } from '../../stores/dialogStore';

import { useTranscriptStore } from '../../stores/transcriptStore';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
        i18n: { changeLanguage: vi.fn() },
    }),
}));

vi.mock('../../stores/transcriptStore', async () => {
    const { create } = await import('zustand');
    const actual = await vi.importActual('../../stores/transcriptStore');

    // Create a functional store for testing
    const useTranscriptStore = create((set) => ({
        config: {

            offlineModelPath: '/test/offline',
            enableITN: true,
            enabledITNModels: ['itn-zh-number'],
            itnRulesOrder: ['itn-zh-number'],
            appLanguage: 'auto',
            language: 'en',
            punctuationModelPath: '',
            vadModelPath: '',
            theme: 'auto',
            font: 'system',
            vadBufferSize: 5
        },
        setConfig: (config: any) => set((state: any) => ({ config: { ...state.config, ...config } })),
    }));

    return {
        ...actual,
        useTranscriptStore
    };
});

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [
        { id: 'test-model', name: 'Test Model', language: 'en', type: 'offline', size: '100MB', description: 'Test', engine: 'onnx', filename: 'test-model' },
        { id: 'test-model-2', name: 'Second Model', language: 'en', type: 'offline', size: '100MB', description: 'Test 2', engine: 'onnx', filename: 'test-model-2' },
        { id: 'test-punct', name: 'Test Punctuation', language: 'en', type: 'punctuation', size: '50MB', description: 'Test Punct', engine: 'onnx', filename: 'test-punct' },
        { id: 'itn-zh-number', name: 'Chinese Number ITN', description: 'Test ITN', filename: 'itn_zh_number.fst', type: 'itn', engine: 'onnx' }
    ],
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false), // Default: Not installed
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn().mockImplementation((id) => Promise.resolve(`/path/to/${id}`)),
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
        // Reset store state
        useTranscriptStore.setState({
            config: {

                offlineModelPath: '/test/offline',
                enableITN: true,
                enabledITNModels: ['itn-zh-number'],
                itnRulesOrder: ['itn-zh-number'],
                appLanguage: 'auto',
                language: 'en',
                punctuationModelPath: '',
                vadModelPath: '',
                theme: 'auto',
                font: 'system',
                vadBufferSize: 5
            }
        });
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
        // Wait, SettingsModelsTab uses ModelCard which likely uses aria-label or text.
        // Assuming "Test Model" is visible.
        // Actually the label might be tricky if not translated properly.
        // But the previous test used `screen.getByLabelText('common.download Test Model')`.

        // Let's rely on previous test logic.
        // ModelCard uses t('common.download') + ' ' + model.name if not installed.
        // Mock t returns key. So "common.download Test Model".
        const downloadBtn = screen.getByLabelText('common.download Test Model');

        // Simulate download
        vi.mocked(modelService.downloadModel).mockImplementation(async (_id, onProgress) => {
            if (onProgress) {
                onProgress(50, 'Downloading...');
            }
            // Add delay to ensure loading state is visible
            await new Promise(resolve => setTimeout(resolve, 100));

            if (onProgress) {
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
        const initialCalls = vi.mocked(modelService.isModelInstalled).mock.calls.length;

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
        await waitFor(() => {
            expect(vi.mocked(modelService.isModelInstalled).mock.calls.length).toBeGreaterThan(initialCalls);
        });
    });

    it('loads a model path when Load is clicked', async () => {
        // Setup: Model is installed
        vi.mocked(modelService.isModelInstalled).mockImplementation((id) => Promise.resolve(id === 'test-model'));

        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        fireEvent.click(screen.getByText('settings.model_hub'));

        const loadBtn = await screen.findByLabelText('settings.load Test Model');
        fireEvent.click(loadBtn);

        expect(modelService.getModelPath).toHaveBeenCalledWith('test-model');

        // Verify store update
        await waitFor(() => {
            expect(useTranscriptStore.getState().config.offlineModelPath).toBe('/path/to/test-model');
        });
    });
});
