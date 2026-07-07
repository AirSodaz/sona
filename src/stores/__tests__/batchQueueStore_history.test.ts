import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useHistoryStore } from '../historyStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { useConfigStore } from '../configStore';
import { historyService } from '../../services/historyService';
import { transcriptionService } from '../../services/transcriptionService';
import { emitAutomationTaskSettled } from '../../services/automationEventBus';

// Mock dependencies
vi.mock('uuid', () => ({
    v4: () => 'test-uuid-123'
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
    invoke: vi.fn()
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(() => Promise.resolve(vi.fn())),
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


        transcribeFile: vi.fn()
    }
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [],
    PRESET_MODELS_MAP: new Map(),
    modelService: {
        getEnabledITNModelPaths: vi.fn(),
        getModelRules: vi.fn(() => ({
            requiresPunctuation: false,
            requiresVad: false,
        })),
    }
}));

vi.mock('../../services/tauri/taskLedger', () => ({
    taskLedgerLoadSnapshot: vi.fn(() => Promise.resolve({ version: 1, updatedAt: null, tasks: [] })),
    taskLedgerPatchTask: vi.fn(() => Promise.resolve({ version: 1, updatedAt: null, tasks: [] })),
    taskLedgerUpsertTask: vi.fn(() => Promise.resolve({ version: 1, updatedAt: null, tasks: [] })),
    taskLedgerRemoveTask: vi.fn(() => Promise.resolve({ version: 1, updatedAt: null, tasks: [] })),
    taskLedgerClearResolved: vi.fn(() => Promise.resolve({ version: 1, updatedAt: null, tasks: [] })),
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        saveImportedFile: vi.fn().mockResolvedValue({
            id: 'history-1',
            timestamp: 1,
            duration: 1,
            audioPath: 'history-1.wav',
            transcriptPath: 'history-1.json',
            title: 'Batch test.wav',
            previewText: 'Hello...',
            searchContent: 'Hello',
            type: 'batch',
            projectId: null,
        }),
        updateTranscript: vi.fn().mockResolvedValue(undefined),
        getAudioUrl: vi.fn().mockResolvedValue('asset:///history/history-1.wav'),
    }
}));

vi.mock('../../services/automationEventBus', () => ({
    emitAutomationTaskSettled: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../services/summaryService', () => ({
    summaryService: {
        persistSummary: vi.fn().mockResolvedValue(undefined),
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

describe('batchQueueStore History Integration', () => {
    beforeEach(() => {
        useBatchQueueStore.getState().clearQueue();
        useHistoryStore.setState({
            items: [],
            isLoading: false,
            error: null,
        });
        useTranscriptStore.getState().setAudioUrl(null);
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.setState({
            sourceHistoryId: null,
            title: '',
            icon: null,
        });
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                streamingModelPath: "/path/to/model",
                batchModelPath: '/path/to/model',
                language: 'en',

                appLanguage: 'en'
            }
        });
        vi.clearAllMocks();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('should save to history after successful transcription', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false
            }
        });
        const file = '/path/to/test.wav';
        const mockSegments = [
            { id: 'seg1', start: 0, end: 1, text: 'Hello', isFinal: true },
            { id: 'seg2', start: 1, end: 2, text: 'World', isFinal: true }
        ];

        // Mock transcription success
        (transcriptionService.transcribeFile as any).mockImplementation(async () => {
            console.log('Mock transcription called');
            return mockSegments;
        });

        // Action: Add file (which auto-starts processing because config is set)
        console.log('Adding files...');
        useBatchQueueStore.getState().addFiles([file]);

        // Wait for async processing
        console.log('Waiting for processing...');
        await new Promise(resolve => setTimeout(resolve, 1000));
        console.log('Wait complete');

        // Assert History Save
        expect(historyService.saveImportedFile).toHaveBeenCalledTimes(1);
        expect(historyService.saveImportedFile).toHaveBeenCalledWith(
            file,
            mockSegments,
            2, // Duration from last segment
            '/tmp/test-uuid-123.wav',
            null,
            'test-uuid-123',
        );

        // Assert Item Status
        const queueState = useBatchQueueStore.getState();
        expect(queueState.queueItems[0].status).toBe('complete');
    });

    it('saves Volcengine batch imports from the original file path without a temp WAV copy', async () => {
        const file = '/path/to/volcengine.wav';
        const mockSegments = [
            { id: 'volc-seg-1', start: 0, end: 2, text: 'Online batch saved', isFinal: true },
        ];

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
                asr: {
                    selections: {
                        live: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                        caption: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                        voiceTyping: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                        batch: {
                            engine: 'online',
                            mode: 'batch',
                            modelId: null,
                            modelPath: '',
                            providerId: 'volcengine-doubao',
                            profileId: 'volcengine-doubao-default',
                        },
                    },
                    providers: {
                        online: {
                            'volcengine-doubao': {
                                apiKey: 'volc-test-key',
                                streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
                                streamingResourceId: 'volc.seedasr.sauc.duration',
                                batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
                                batchResourceId: 'volc.bigasr.auc_turbo',
                            },
                        },
                    },
                },
            },
        });
        (transcriptionService.transcribeFile as any).mockResolvedValue(mockSegments);

        useBatchQueueStore.getState().addFiles([file]);

        await new Promise(resolve => setTimeout(resolve, 1000));

        expect(historyService.saveImportedFile).toHaveBeenCalledWith(
            file,
            mockSegments,
            2,
            undefined,
            null,
            'test-uuid-123',
        );
        expect(useBatchQueueStore.getState().queueItems[0]).toEqual(expect.objectContaining({
            historyId: 'history-1',
            status: 'complete',
        }));
    });

    it('keeps the active editor title aligned with the saved history title for batch imports', async () => {
        const file = '/path/to/meeting.wav';
        const mockSegments = [
            { id: 'seg1', start: 0, end: 1, text: 'Hello', isFinal: true },
        ];
        let resolveSave!: (value: { id: string; title: string; projectId: null }) => void;

        (transcriptionService.transcribeFile as any).mockResolvedValue(mockSegments);
        (historyService.saveImportedFile as any).mockImplementation(() => new Promise((resolve) => {
            resolveSave = resolve;
        }));

        useBatchQueueStore.getState().addFiles([file]);

        expect(useTranscriptStore.getState().title).toBe('meeting.wav');

        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(useTranscriptStore.getState().title).toBe('meeting.wav');

        resolveSave({
            id: 'history-1',
            title: 'Batch meeting.wav',
            projectId: null,
        });

        await new Promise((resolve) => setTimeout(resolve, 0));

        expect(useTranscriptStore.getState().sourceHistoryId).toBe('history-1');
        expect(useTranscriptStore.getState().title).toBe('Batch meeting.wav');

        const queueItemId = useBatchQueueStore.getState().queueItems[0].id;
        useBatchQueueStore.getState().setActiveItem(null);
        expect(useTranscriptStore.getState().title).toBe('');

        useBatchQueueStore.getState().setActiveItem(queueItemId);
        expect(useTranscriptStore.getState().title).toBe('Batch meeting.wav');
    });

    it('syncs the active editor audio URL from the saved history item after batch import completes', async () => {
        const file = '/path/to/player.wav';
        const mockSegments = [
            { id: 'seg1', start: 0, end: 1, text: 'Hello', isFinal: true },
        ];

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
            }
        });
        (transcriptionService.transcribeFile as any).mockResolvedValue(mockSegments);
        vi.mocked(historyService.saveImportedFile).mockResolvedValueOnce({
            id: 'history-player',
            timestamp: 1,
            duration: 1,
            audioPath: 'history-player.wav',
            transcriptPath: 'history-player.json',
            title: 'Batch player.wav',
            previewText: 'Hello...',
            searchContent: 'Hello',
            type: 'batch',
            projectId: null,
        });
        vi.mocked(historyService.getAudioUrl).mockResolvedValueOnce('asset:///history/history-player.wav');

        useBatchQueueStore.getState().addFiles([file]);

        await new Promise((resolve) => setTimeout(resolve, 1000));

        expect(historyService.getAudioUrl).toHaveBeenCalledWith('history-player');
        expect(useTranscriptStore.getState()).toEqual(expect.objectContaining({
            sourceHistoryId: 'history-player',
            title: 'Batch player.wav',
            audioUrl: 'asset:///history/history-player.wav',
        }));
    });

    it('does not fall back to an external asset URL when saved history audio cannot be resolved', async () => {
        const file = '/path/to/fallback.wav';
        const mockSegments = [
            { id: 'seg1', start: 0, end: 1, text: 'Hello', isFinal: true },
        ];

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
            }
        });
        (transcriptionService.transcribeFile as any).mockResolvedValue(mockSegments);
        vi.mocked(historyService.saveImportedFile).mockResolvedValueOnce({
            id: 'history-fallback',
            timestamp: 1,
            duration: 1,
            audioPath: 'history-fallback.wav',
            transcriptPath: 'history-fallback.json',
            title: 'Batch fallback.wav',
            previewText: 'Hello...',
            searchContent: 'Hello',
            type: 'batch',
            projectId: null,
        });
        vi.mocked(historyService.getAudioUrl).mockResolvedValueOnce(null);

        useBatchQueueStore.getState().addFiles([file]);

        await new Promise((resolve) => setTimeout(resolve, 1000));

        expect(useTranscriptStore.getState()).toEqual(expect.objectContaining({
            sourceHistoryId: 'history-fallback',
            title: 'Batch fallback.wav',
            audioUrl: null,
        }));
    });

    it('updates in-memory history metadata immediately when a saved batch item is rewritten later', async () => {
        vi.mocked(historyService.updateTranscript).mockResolvedValueOnce({
            id: 'history-1',
            timestamp: 1,
            duration: 1,
            audioPath: 'history-1.wav',
            transcriptPath: 'history-1.json',
            title: 'Batch post-process.wav',
            previewText: 'Hello polished...',
            searchContent: 'Hello polished',
            type: 'batch',
            projectId: null,
            status: 'complete',
        });

        useHistoryStore.setState({
            items: [
                {
                    id: 'history-1',
                    timestamp: 1,
                    duration: 1,
                    audioPath: 'history-1.wav',
                    transcriptPath: 'history-1.json',
                    title: 'Batch post-process.wav',
                    previewText: 'Hello...',
                    searchContent: 'Hello',
                    type: 'batch',
                    projectId: null,
                },
            ],
            isLoading: false,
            error: null,
        });

        await useHistoryStore.getState().updateTranscript('history-1', [
            { id: 'seg1', start: 0, end: 1, text: 'Hello polished', isFinal: true },
        ]);

        expect(historyService.updateTranscript).toHaveBeenCalledWith('history-1', [
            { id: 'seg1', start: 0, end: 1, text: 'Hello polished', isFinal: true },
        ]);
        expect(useHistoryStore.getState().items).toEqual([
            expect.objectContaining({
                id: 'history-1',
                previewText: 'Hello polished...',
                searchContent: 'Hello polished',
            }),
        ]);
    });

    it('reuses the existing history draft when a recovered batch item is resumed', async () => {
        const finalSegments = [
            { id: 'seg1', start: 0, end: 1, text: 'Recovered final', isFinal: true },
        ];

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
            }
        });
        (transcriptionService.transcribeFile as any).mockResolvedValue(finalSegments);

        useBatchQueueStore.getState().enqueueRecoveredItems([{
            id: 'recovery-1',
            filename: 'recovered.wav',
            filePath: '/path/to/recovered.wav',
            source: 'batch_import',
            resolution: 'pending',
            progress: 45,
            segments: [
                { id: 'draft-1', start: 0, end: 1, text: 'Draft text', isFinal: true },
            ],
            projectId: null,
            historyId: 'history-1',
            historyTitle: 'Recovered draft',
            lastKnownStage: 'transcribing',
            updatedAt: 1,
            hasSourceFile: true,
            canResume: true,
        }]);

        await new Promise((resolve) => setTimeout(resolve, 1000));

        expect(historyService.saveImportedFile).not.toHaveBeenCalled();
        expect(historyService.updateTranscript).toHaveBeenCalledWith('history-1', finalSegments);
        expect(useBatchQueueStore.getState().queueItems[0]).toEqual(expect.objectContaining({
            historyId: 'history-1',
            historyTitle: 'Recovered draft',
            status: 'complete',
        }));
    });

    it('passes the latest known stage through settled automation success payloads', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
            }
        });
        (transcriptionService.transcribeFile as any).mockResolvedValue([
            { id: 'seg1', start: 0, end: 1, text: 'Done', isFinal: true },
        ]);
        (historyService.saveImportedFile as any).mockResolvedValueOnce(null);

        useBatchQueueStore.setState({
            queueItems: [{
                id: 'automation-success-1',
                filename: 'automated.wav',
                filePath: '/path/to/automated.wav',
                status: 'pending',
                progress: 0,
                segments: [],
                projectId: null,
                origin: 'automation',
                automationRuleId: 'rule-1',
                automationRuleName: 'Automation Rule',
                sourceFingerprint: 'fp-success',
                fileStat: {
                    size: 42,
                    mtimeMs: 1000,
                },
                lastKnownStage: 'queued',
            }],
            activeItemId: null,
            isQueueProcessing: false,
        });

        await useBatchQueueStore.getState()._processItem('automation-success-1');

        expect(emitAutomationTaskSettled).toHaveBeenCalledWith(expect.objectContaining({
            ruleId: 'rule-1',
            sourceFingerprint: 'fp-success',
            status: 'complete',
            stage: 'transcribing',
        }));
    });

    it('passes the latest known stage through settled automation failure payloads', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                enableTimeline: false,
            }
        });
        (transcriptionService.transcribeFile as any).mockRejectedValue(new Error('Transcription failed'));

        useBatchQueueStore.setState({
            queueItems: [{
                id: 'automation-failure-1',
                filename: 'automated-fail.wav',
                filePath: '/path/to/automated-fail.wav',
                status: 'pending',
                progress: 0,
                segments: [],
                projectId: null,
                origin: 'automation',
                automationRuleId: 'rule-1',
                automationRuleName: 'Automation Rule',
                sourceFingerprint: 'fp-failure',
                fileStat: {
                    size: 84,
                    mtimeMs: 1001,
                },
                lastKnownStage: 'queued',
            }],
            activeItemId: null,
            isQueueProcessing: false,
        });

        await useBatchQueueStore.getState()._processItem('automation-failure-1');

        expect(emitAutomationTaskSettled).toHaveBeenCalledWith(expect.objectContaining({
            ruleId: 'rule-1',
            sourceFingerprint: 'fp-failure',
            status: 'error',
            stage: 'transcribing',
            errorMessage: 'Transcription failed',
        }));
    });
});
