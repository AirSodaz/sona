import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within, act } from '@testing-library/react';
import { BatchImport } from '../BatchImport';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { transcriptionService } from '../../services/transcriptionService';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: vi.fn((path) => `asset://${path}`),
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
    tempDir: vi.fn(() => Promise.resolve('/tmp')),
    join: vi.fn((...args) => Promise.resolve(args.join('/'))),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => { }),
}));

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        listen: vi.fn().mockResolvedValue(() => { }),
    }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
    message: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(() => Promise.resolve(false)),
    remove: vi.fn(() => Promise.resolve()),
    mkdir: vi.fn(() => Promise.resolve()),
    writeTextFile: vi.fn(() => Promise.resolve()),
    readTextFile: vi.fn(() => Promise.resolve('')),
    BaseDirectory: { AppData: 1, Resource: 2, AppLocalData: 3 },
}));

// Mock transcription service
vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
        setCtcModelPath: vi.fn(),
        setSourceFilePath: vi.fn(), // Added mock
        transcribeFile: vi.fn(),
    }
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn().mockResolvedValue(['/itn/path']),
    }
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'mock-history-id' }),
    }
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: any) => {
            if (key === 'batch.supports') return `Supports: ${options.formats}`;
            if (key === 'batch.queue_title') return `Queue (${options.count})`;
            return key;
        },
    }),
}));

describe('BatchImport Integration', () => {
    beforeEach(() => {
        // Reset stores
        useTranscriptStore.setState({
            config: {

                offlineModelPath: '/mock/offline/model',
                punctuationModelPath: '',
                enableITN: false,
                enabledITNModels: [],
                itnRulesOrder: [],
                theme: 'auto',
                font: 'system',
                language: 'en',
                appLanguage: 'auto'
            },
            segments: [],
            audioUrl: null
        });

        useBatchQueueStore.setState({
            queueItems: [],
            activeItemId: null,
            isQueueProcessing: false,
        });

        vi.clearAllMocks();
    });

    it('renders drop zone initially', () => {
        render(<BatchImport />);
        expect(screen.getByText('batch.drop_title')).toBeDefined();
        expect(screen.getByText('batch.drop_desc')).toBeDefined();
    });

    it('adds files to queue and starts processing automatically', async () => {
        // Mock transcribeFile to simulate progress with delay
        const mockTranscribe = vi.mocked(transcriptionService.transcribeFile).mockImplementation(
            async (_path, onProgress, onSegment) => {
                if (onProgress) onProgress(10);
                if (onSegment) onSegment({ id: '1', start: 0, end: 1, text: 'Test', isFinal: true });
                // Wait to allow assertion of processing state
                await new Promise(resolve => setTimeout(resolve, 100));
                return [{ id: '1', start: 0, end: 1, text: 'Test', isFinal: true }];
            }
        );

        render(<BatchImport />);

        const { addFiles } = useBatchQueueStore.getState();

        await act(async () => {
            addFiles(['/path/to/test.wav']);
        });

        // 1. Check if sidebar appears
        await waitFor(() => {
            expect(screen.getByText('Queue (1)')).toBeDefined();
        });

        // 2. Check if processing view appears
        await waitFor(() => {
            expect(screen.getAllByText('batch.processing_title').length).toBeGreaterThan(0);
        });

        // 3. Check progress bar updates
        // Look within the processing view container to avoid ambiguity with sidebar
        const processingView = screen.getAllByText('batch.processing_title')[0].closest('.batch-queue-processing');
        if (!processingView) throw new Error('Processing view not found');
        const progress = within(processingView as HTMLElement).getByRole('progressbar');
        expect(progress.getAttribute('aria-valuenow')).toBe('10');

        // 4. Check if service was called
        expect(mockTranscribe).toHaveBeenCalled();
    });

    it('shows error state when transcription fails', async () => {
        vi.mocked(transcriptionService.transcribeFile).mockRejectedValue(new Error('Mock Error'));

        render(<BatchImport />);

        const { addFiles } = useBatchQueueStore.getState();

        await act(async () => {
            addFiles(['/path/to/fail.wav']);
        });

        await waitFor(() => {
            expect(screen.getAllByText('batch.file_failed').length).toBeGreaterThan(0);
        });

        // Error details
        const sidebar = screen.getByRole('list', { name: /Queue/ });
        expect(within(sidebar).getByText('batch.file_failed')).toBeDefined();
    });

    it('can remove items from queue', async () => {
        render(<BatchImport />);
        const { addFiles } = useBatchQueueStore.getState();

        await act(async () => {
            addFiles(['/path/to/file1.wav', '/path/to/file2.wav']);
        });

        // Wait for list
        const sidebar = await screen.findByRole('list', { name: /Queue/ });

        // Check initial length
        await waitFor(() => {
            expect(within(sidebar).getAllByRole('listitem')).toHaveLength(2);
        });

        // Find remove button for first item
        const removeBtns = within(sidebar).getAllByLabelText('common.delete_item');
        fireEvent.click(removeBtns[0]);

        await waitFor(() => {
            expect(within(sidebar).getAllByRole('listitem')).toHaveLength(1);
        });
    });

    it('allows clearing the queue', async () => {
        render(<BatchImport />);
        const { addFiles } = useBatchQueueStore.getState();

        await act(async () => {
            addFiles(['/path/to/file1.wav']);
        });

        await waitFor(() => {
            expect(screen.getByText('Queue (1)')).toBeDefined();
        });

        const clearBtn = screen.getByLabelText('batch.clear_queue');
        fireEvent.click(clearBtn);

        await waitFor(() => {
            expect(screen.queryByText('Queue (1)')).toBeNull();
            // Should revert to drop zone
            expect(screen.getByText('batch.drop_title')).toBeDefined();
        });
    });

    it('allows adding more files while queue is processing', async () => {
        // Setup initial state with processing item
        useBatchQueueStore.setState({
            queueItems: [{
                id: '1',
                filename: 'processing.wav',
                filePath: '/path/to/processing.wav',
                status: 'processing',
                progress: 50,
                segments: [],
                audioUrl: 'asset:///path/to/processing.wav'
            }],
            activeItemId: '1',
            isQueueProcessing: true
        });

        render(<BatchImport />);

        // Check if processing view is shown
        expect(screen.getByText('batch.processing_title')).toBeDefined();

        // Check if "Add more files" button is present and NOT disabled
        const addButton = screen.getByRole('button', { name: 'batch.add_more_files' }) as HTMLButtonElement;
        expect(addButton).toBeDefined();
        expect(addButton.disabled).toBe(false);

        // Verify clicking it triggers file dialog
        fireEvent.click(addButton);
    });
});
