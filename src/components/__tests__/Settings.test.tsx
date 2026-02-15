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
        { id: 'test-punct', name: 'Test Punctuation', language: 'en', type: 'punctuation', size: '50MB', description: 'Test Punct', engine: 'onnx', filename: 'test-punct' }
    ],
    ITN_MODELS: [
        { id: 'itn-zh-number', name: 'Chinese Number ITN', description: 'Test ITN', filename: 'itn_zh_number.fst' }
    ],
    modelService: {
        isModelInstalled: vi.fn().mockResolvedValue(false), // Default: Not installed
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        checkHardware: vi.fn().mockResolvedValue({ compatible: true }),
        downloadModel: vi.fn(),
        getModelPath: vi.fn().mockImplementation((id) => Promise.resolve(`/path/to/${id}`)),
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

        // Check if path was set in state (switch to Local Path tab to verify)
        fireEvent.click(screen.getByText('settings.local_path'));

        // Expect Dropdown to show "Test Model"
        await waitFor(() => {
            expect(screen.getByText('Test Model')).toBeDefined();
        });
    });

    it('auto-saves configuration on change', async () => {
        // Setup: Ensure both models are "installed" so they are selectable in the dropdown
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(true);

        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Switch to Local Path tab
        fireEvent.click(screen.getByText('settings.local_path'));

        // Open Dropdown
        const trigger = document.getElementById('settings-offline-path');
        expect(trigger).toBeDefined();
        if (trigger) fireEvent.click(trigger);

        // Select 'Second Model'
        const option = screen.getByText('Second Model');
        fireEvent.click(option);

        // Verify setConfig was called with new value (by checking state update)
        await waitFor(() => {
            expect(useTranscriptStore.getState().config).toMatchObject({
                offlineModelPath: '/path/to/test-model-2'
            });
        });
    });

    it('prevents enabling switch if model is not installed', async () => {
        // Setup: No models installed
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);
        vi.spyOn(useDialogStore.getState(), 'alert').mockResolvedValue();

        render(<Settings isOpen={true} onClose={onClose} />);
        await waitFor(() => expect(modelService.isModelInstalled).toHaveBeenCalled());

        // Switch to Local Path tab
        fireEvent.click(screen.getByText('settings.local_path'));

        // Try to toggle Punctuation switch (which corresponds to 'settings.punctuation_path_label')
        const label = screen.getByText('settings.punctuation_path_label');
        // The switch is a sibling of the label in the flex container
        const row = label.closest('div');
        const switchContainer = row?.querySelector('.switch-container');

        expect(switchContainer).toBeDefined();

        if (switchContainer) {
            fireEvent.click(switchContainer);

            // Verify alert was called
            expect(useDialogStore.getState().alert).toHaveBeenCalled();

            // Verify path was NOT set
            expect(useTranscriptStore.getState().config.punctuationModelPath).toBe('');
        }
    });
});
