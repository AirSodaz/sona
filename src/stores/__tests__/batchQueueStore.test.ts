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

    it('creates new batch and automation ledger records when recovered items are enqueued', () => {
        useBatchQueueStore.getState().enqueueRecoveredItems([
            {
                id: 'recovery-batch-1',
                filename: 'batch.wav',
                filePath: '/batch.wav',
                source: 'batch_import',
                resolution: 'pending',
                progress: 25,
                segments: [],
                projectId: null,
                lastKnownStage: 'transcribing',
                updatedAt: 100,
                hasSourceFile: true,
                canResume: true,
            },
            {
                id: 'recovery-automation-1',
                filename: 'automation.wav',
                filePath: '/automation.wav',
                source: 'automation',
                resolution: 'pending',
                progress: 60,
                segments: [],
                projectId: 'project-1',
                lastKnownStage: 'transcribing',
                updatedAt: 101,
                hasSourceFile: true,
                canResume: true,
                automationRuleId: 'rule-1',
                sourceFingerprint: 'fp-1',
            },
        ]);

        expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(expect.objectContaining({
            id: 'batch-recovery-batch-1',
            kind: 'batchImport',
            status: 'pending',
            title: 'batch.wav',
        }));
        expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(expect.objectContaining({
            id: 'batch-recovery-automation-1',
            kind: 'automation',
            status: 'pending',
            title: 'automation.wav',
        }));
    });

    it('records recovered item failures on the new batch ledger record', () => {
        useBatchQueueStore.setState({
            queueItems: [
                {
                    id: 'recovery-automation-1',
                    recoveryId: 'recovery-automation-1',
                    filename: 'automation.wav',
                    filePath: '/automation.wav',
                    status: 'pending',
                    progress: 0,
                    segments: [],
                    audioUrl: 'asset:///automation.wav',
                    projectId: null,
                    origin: 'automation',
                },
            ],
            activeItemId: 'recovery-automation-1',
        });

        useBatchQueueStore.getState().setItemError('recovery-automation-1', 'still broken');

        expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenCalledWith('batch-recovery-automation-1', expect.objectContaining({
            status: 'failed',
            errorMessage: 'still broken',
            retryable: true,
        }));
    });

    it('processes queue items when the batch ASR slot is Volcengine Doubao with no local model path', async () => {
        const { transcriptionService } = await import('../../services/transcriptionService');
        const { processBatchQueueItem } = await import('../../services/batch/batchItemProcessor');
        vi.mocked(transcriptionService.transcribeFile).mockResolvedValue([
            {
                id: 'volc-1',
                text: '云端结果',
                start: 0,
                end: 1,
                isFinal: true,
            } as any,
        ]);
        const item = {
            id: '1',
            filename: 'cloud.wav',
            filePath: '/cloud.wav',
            status: 'pending',
            progress: 0,
            segments: [],
            audioUrl: 'asset:///cloud.wav',
            projectId: null,
        } as any;
        const config = {
            language: 'auto',
            enableITN: true,
            asr: {
                selections: {
                    live: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    caption: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    voiceTyping: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    batch: {
                        engine: 'volcengine-doubao',
                        mode: 'offline',
                        modelId: null,
                        modelPath: '',
                        providerId: 'volcengine-doubao',
                        profileId: 'volcengine-doubao-default',
                    },
                },
                providers: {
                    volcengineDoubao: {
                        apiKey: 'volc-test-key',
                        streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
                        streamingResourceId: 'volc.seedasr.sauc.duration',
                        batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
                        batchResourceId: 'volc.bigasr.auc_turbo',
                    },
                },
            },
        } as any;
        const updateStatus = vi.fn();
        const updateSegments = vi.fn();

        await processBatchQueueItem({
            item,
            config,
            callbacks: {
                updateStatus,
                updateSegments,
                onHistorySaved: vi.fn(),
                onExportComplete: vi.fn(),
                isActiveItem: () => false,
                isCancelRequested: () => false,
            },
        });

        expect(transcriptionService.setModelPath).not.toHaveBeenCalledWith('');
        expect(transcriptionService.transcribeFile).toHaveBeenCalledWith(
            '/cloud.wav',
            expect.any(Function),
            expect.any(Function),
            undefined,
            expect.any(String),
            config,
        );
        expect(updateSegments).toHaveBeenCalledWith([
            expect.objectContaining({ text: '云端结果' }),
        ]);
        expect(updateStatus).toHaveBeenCalledWith('processing', 0, 'transcribing');
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
