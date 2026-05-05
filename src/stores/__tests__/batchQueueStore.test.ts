import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

const taskLedgerContext = vi.hoisted(() => ({
    upsertTaskLedgerRecord: vi.fn(),
    patchTaskLedgerRecord: vi.fn(),
    isTaskLedgerCancelRequested: vi.fn(() => false),
}));

// Mock dependencies
vi.mock('uuid', () => ({
    v4: () => 'test-uuid-123'
}));

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

vi.mock('../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),

        transcribeFile: vi.fn()
    }
}));

vi.mock('../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn()
    }
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

vi.mock('../../services/taskLedgerRuntime', () => ({
    buildBatchTaskLedgerRecord: (item: any, status = 'pending') => ({
        id: `batch-${item.id}`,
        kind: item.origin === 'automation' ? 'automation' : 'batchImport',
        status,
        title: item.filename,
        progress: item.progress,
        createdAt: 100,
        updatedAt: 100,
        retryable: true,
        cancelable: true,
        recoverable: false,
        filePath: item.filePath,
    }),
    createBatchTaskLedgerId: (id: string) => `batch-${id}`,
    upsertTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(taskLedgerContext.upsertTaskLedgerRecord, undefined, args),
    patchTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(taskLedgerContext.patchTaskLedgerRecord, undefined, args),
    isTaskLedgerCancelRequested: (...args: unknown[]) => Reflect.apply(taskLedgerContext.isTaskLedgerCancelRequested, undefined, args),
}));

describe('batchQueueStore', () => {
    beforeEach(() => {
        useBatchQueueStore.getState().clearQueue();
        useTranscriptStore.getState().setAudioUrl(null);
        useTranscriptStore.getState().clearSegments();
        vi.clearAllMocks();
        taskLedgerContext.isTaskLedgerCancelRequested.mockReturnValue(false);
    });

    it('should automatically set active item and sync to transcript store when adding files', () => {
        const files = ['/path/to/test.wav'];

        // Action
        useBatchQueueStore.getState().addFiles(files);

        // Assert Queue State
        const queueState = useBatchQueueStore.getState();
        expect(queueState.queueItems).toHaveLength(1);
        expect(queueState.activeItemId).toBe('test-uuid-123');

        // Assert Transcript Store State
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBe('asset:///path/to/test.wav');
        expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(expect.objectContaining({
            id: 'batch-test-uuid-123',
            kind: 'batchImport',
            status: 'pending',
            title: 'test.wav',
            progress: 0,
            filePath: '/path/to/test.wav',
        }));
    });

    it('records task ledger progress and failures for queue items', () => {
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav', projectId: null },
            ],
            activeItemId: '1',
        });

        useBatchQueueStore.getState().updateItemStatus('1', 'processing', 55, 'transcribing');
        useBatchQueueStore.getState().setItemError('1', 'broken');

        expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenNthCalledWith(1, 'batch-1', expect.objectContaining({
            status: 'running',
            progress: 55,
            stage: 'transcribing',
        }));
        expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenNthCalledWith(2, 'batch-1', expect.objectContaining({
            status: 'failed',
            errorMessage: 'broken',
            retryable: true,
        }));
    });

    it('should sync to transcript store when removing the active item', () => {
        // Setup
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav', projectId: null },
                { id: '2', filename: '2.wav', filePath: '/2.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///2.wav', projectId: null }
            ],
            activeItemId: '1'
        });

        // Simulate what happens when 1 is active (manually sync for setup)
        useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

        // Action: Remove active item
        useBatchQueueStore.getState().removeItem('1');

        // Assert Queue State
        const queueState = useBatchQueueStore.getState();
        expect(queueState.activeItemId).toBe('2');

        // Assert Transcript Store State
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBe('asset:///2.wav');
    });

    it('should NOT sync to transcript store when removing a non-active item', () => {
        // Setup
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav', projectId: null },
                { id: '2', filename: '2.wav', filePath: '/2.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///2.wav', projectId: null }
            ],
            activeItemId: '1'
        });

        // Simulate what happens when 1 is active
        useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

        // Action: Remove non-active item
        useBatchQueueStore.getState().removeItem('2');

        // Assert Queue State
        const queueState = useBatchQueueStore.getState();
        expect(queueState.activeItemId).toBe('1'); // Should still be 1
        expect(queueState.queueItems).toHaveLength(1);
        expect(queueState.queueItems[0].id).toBe('1');

        // Assert Transcript Store State (Should NOT change)
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBe('asset:///1.wav');
    });

    it('should clear transcript store when removing the last active item', () => {
        // Setup
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav', projectId: null }
            ],
            activeItemId: '1'
        });

        // Simulate what happens when 1 is active
        useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

        // Action: Remove active item (the only one)
        useBatchQueueStore.getState().removeItem('1');

        // Assert Queue State
        const queueState = useBatchQueueStore.getState();
        expect(queueState.activeItemId).toBeNull();
        expect(queueState.queueItems).toHaveLength(0);

        // Assert Transcript Store State (Should be cleared)
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBeNull();
        expect(transcriptState.segments).toHaveLength(0);
    });
});
