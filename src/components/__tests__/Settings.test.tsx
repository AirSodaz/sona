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

    it('auto-saves configuration on change', async () => {
        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Switch to Local Path tab
        fireEvent.click(screen.getByText('settings.local_path'));

        // Find the input associated with the label. 
        // Note: In the actual component, the label has htmlFor="settings-streaming-path" and input has id="settings-streaming-path".
        // Testing library's getByLabelText should work if id matches. 
        // However, standard getByLabelText might require exact match of translation key if I'm mocking translation to return key.
        // The mock returns key as translation.
        // Label text in SettingsLocalTab is t('settings.streaming_path_label') which becomes "settings.streaming_path_label".

        const input = screen.getByLabelText('settings.offline_path_label');
        fireEvent.change(input, { target: { value: '/new/offline/path' } });

        // Verify setConfig was called with new value (by checking state update)
        expect(useTranscriptStore.getState().config).toMatchObject({
            offlineModelPath: '/new/offline/path'
        });
    });
});
