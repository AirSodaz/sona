import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { transcriptionService } from '../../services/transcriptionService';
import { useTranscriptStore } from '../transcriptStore';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`
}));

vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
        setCtcModelPath: vi.fn(),
        setSourceFilePath: vi.fn(),
        transcribeFile: vi.fn(),
    }
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'mock-history-id' }),
    }
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn().mockResolvedValue([]),
    }
}));

describe('batchQueueStore Concurrency', () => {
    beforeEach(() => {
        useBatchQueueStore.setState({
            queueItems: [],
            activeItemId: null,
            isQueueProcessing: false
        });
        useTranscriptStore.setState({
            config: {

                offlineModelPath: '/mock/model',
                language: 'en',
                appLanguage: 'auto',
                maxConcurrent: 2
            }
        });
        vi.clearAllMocks();
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('should process multiple items up to maxConcurrent', async () => {
        const store = useBatchQueueStore.getState();

        // Mock transcribeFile to return a promise that we control
        let resolveTask1: (val: any) => void;
        let resolveTask2: (val: any) => void;
        let resolveTask3: (val: any) => void;
        const task1Promise = new Promise(r => resolveTask1 = r);
        const task2Promise = new Promise(r => resolveTask2 = r);
        const task3Promise = new Promise(r => resolveTask3 = r);

        const tasks = {
            'file1.wav': task1Promise,
            'file2.wav': task2Promise,
            'file3.wav': task3Promise
        };

        (transcriptionService.transcribeFile as any).mockImplementation((filePath: string) => {
            const filename = filePath.split(/[/\\]/).pop() || '';
            return tasks[filename as keyof typeof tasks] || Promise.resolve([]);
        });

        // Add 3 files
        store.addFiles(['/path/to/file1.wav', '/path/to/file2.wav', '/path/to/file3.wav']);

        // Wait a bit for the async processQueue to kick in
        await new Promise(r => setTimeout(r, 50));

        // Check status
        const state = useBatchQueueStore.getState();
        const processingItems = state.queueItems.filter(i => i.status === 'processing');
        const pendingItems = state.queueItems.filter(i => i.status === 'pending');

        expect(processingItems.length).toBe(2); // Should cap at 2
        expect(pendingItems.length).toBe(1);

        // Finish task 1
        resolveTask1!([{ id: '1', start: 0, end: 1, text: 'test', isFinal: true }]);

        // Wait for state update and next task trigger
        await new Promise(r => setTimeout(r, 50));

        const stateAfter1 = useBatchQueueStore.getState();
        const processingAfter1 = stateAfter1.queueItems.filter(i => i.status === 'processing');
        const completeAfter1 = stateAfter1.queueItems.filter(i => i.status === 'complete');

        expect(completeAfter1.length).toBe(1);
        expect(processingAfter1.length).toBe(2); // file2 still running, file3 started

        // Finish remaining
        resolveTask2!([{ id: '2', start: 0, end: 1, text: 'test', isFinal: true }]);
        resolveTask3!([{ id: '3', start: 0, end: 1, text: 'test', isFinal: true }]);

        await new Promise(r => setTimeout(r, 50));

        const finalState = useBatchQueueStore.getState();
        expect(finalState.queueItems.every(i => i.status === 'complete')).toBe(true);
        expect(finalState.isQueueProcessing).toBe(false);
    });

    it('should respect maxConcurrent config change', async () => {
        useTranscriptStore.setState({
            config: {
                ...useTranscriptStore.getState().config,
                maxConcurrent: 3
            }
        });

        const store = useBatchQueueStore.getState();

        // Mock simple delay
        (transcriptionService.transcribeFile as any).mockImplementation(async () => {
            await new Promise(r => setTimeout(r, 10));
            return [];
        });

        store.addFiles(['1.wav', '2.wav', '3.wav', '4.wav']);

        await new Promise(r => setTimeout(r, 5));

        const state = useBatchQueueStore.getState();
        const processing = state.queueItems.filter(i => i.status === 'processing');

        expect(processing.length).toBe(3);
    });
});
