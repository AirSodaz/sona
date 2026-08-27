import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { transcriptionService } from '../../services/transcriptionService';
import { useConfigStore } from '../configStore';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
    invoke: vi.fn()
}));

vi.mock('@tauri-apps/api/path', () => ({
    tempDir: vi.fn(() => Promise.resolve('/tmp')),
    join: vi.fn((...args) => Promise.resolve(args.join('/'))),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(() => Promise.resolve(false)),
    remove: vi.fn(() => Promise.resolve()),
    mkdir: vi.fn(() => Promise.resolve()),
    writeTextFile: vi.fn(() => Promise.resolve()),
    readTextFile: vi.fn(() => Promise.resolve('')),
    BaseDirectory: { AppData: 1, Resource: 2, AppLocalData: 3 },
}));

vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),


        transcribeFile: vi.fn(),
    }
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'mock-history-id', projectId: null }),
        updateTranscript: vi.fn().mockResolvedValue(undefined),
        deleteSummary: vi.fn().mockResolvedValue(undefined),
    }
}));

vi.mock('../../services/tauri/taskLedger', () => ({
    taskLedgerUpsertTask: vi.fn().mockResolvedValue({ version: 1, updatedAt: null, tasks: [] }),
    taskLedgerPatchTask: vi.fn().mockResolvedValue({ version: 1, updatedAt: null, tasks: [] }),
    taskLedgerRemoveTask: vi.fn().mockResolvedValue({ version: 1, updatedAt: null, tasks: [] }),
    taskLedgerClearResolved: vi.fn().mockResolvedValue({ version: 1, updatedAt: null, tasks: [] }),
    taskLedgerLoadSnapshot: vi.fn().mockResolvedValue({ version: 1, updatedAt: null, tasks: [] }),
}));

vi.mock('../projectStore', () => ({
    useProjectStore: {
        getState: () => ({
            activeProjectId: null,
            getActiveProject: vi.fn(() => null),
            getProjectById: vi.fn(() => null),
            setActiveProjectId: vi.fn().mockResolvedValue(undefined),
        }),
    },
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [],
    PRESET_MODELS_MAP: new Map(),
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
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                streamingModelPath: "/path/to/model",
                batchModelPath: '/mock/model',
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

        const task1 = Promise.withResolvers<Array<{ id: string; start: number; end: number; text: string; isFinal: boolean }>>();
        const task2 = Promise.withResolvers<Array<{ id: string; start: number; end: number; text: string; isFinal: boolean }>>();
        const task3 = Promise.withResolvers<Array<{ id: string; start: number; end: number; text: string; isFinal: boolean }>>();

        const tasks: Record<string, Promise<Array<{ id: string; start: number; end: number; text: string; isFinal: boolean }>>> = {
            'file1.wav': task1.promise,
            'file2.wav': task2.promise,
            'file3.wav': task3.promise,
        };

        vi.mocked(transcriptionService.transcribeFile).mockImplementation((filePath: string) => {
            const filename = filePath.split(/[/\\]/).pop() || '';
            return (tasks[filename] || Promise.resolve([])) as unknown as never;
        });

        // Add 3 files
        store.addFiles(['/path/to/file1.wav', '/path/to/file2.wav', '/path/to/file3.wav']);

        // Wait for concurrency limit to be reached
        await vi.waitFor(() => {
            const state = useBatchQueueStore.getState();
            const processingItems = state.queueItems.filter(i => i.status === 'processing');
            expect(processingItems.length).toBe(2); // Should cap at 2
        });

        expect(useBatchQueueStore.getState().queueItems.filter(i => i.status === 'pending').length).toBe(1);

        // Finish task 1
        task1.resolve([{ id: '1', start: 0, end: 1, text: 'test', isFinal: true }]);

        // Wait for state update and next task trigger
        await vi.waitFor(() => {
            const state = useBatchQueueStore.getState();
            expect(state.queueItems.filter(i => i.status === 'complete').length).toBe(1);
            expect(state.queueItems.filter(i => i.status === 'processing').length).toBe(2); // file2 still running, file3 started
        });

        // Finish remaining
        task2.resolve([{ id: '2', start: 0, end: 1, text: 'test', isFinal: true }]);
        task3.resolve([{ id: '3', start: 0, end: 1, text: 'test', isFinal: true }]);

        await vi.waitFor(() => {
            const finalState = useBatchQueueStore.getState();
            expect(finalState.queueItems.every(i => i.status === 'complete')).toBe(true);
            expect(finalState.isQueueProcessing).toBe(false);
        });
    });

    it('should respect maxConcurrent config change', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                maxConcurrent: 3,
            },
        });

        const store = useBatchQueueStore.getState();

        const deferred = Promise.withResolvers<never[]>();
        vi.mocked(transcriptionService.transcribeFile).mockImplementation(() => deferred.promise as unknown as never);

        store.addFiles(['1.wav', '2.wav', '3.wav', '4.wav']);

        await vi.waitFor(() => {
            const state = useBatchQueueStore.getState();
            const processing = state.queueItems.filter(i => i.status === 'processing');
            expect(processing.length).toBe(3);
        });

        deferred.resolve([]);
    });
});
