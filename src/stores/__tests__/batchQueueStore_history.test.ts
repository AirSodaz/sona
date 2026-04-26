import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore } from '../transcriptStore';
import { useConfigStore } from '../configStore';
import { historyService } from '../../services/historyService';
import { transcriptionService } from '../../services/transcriptionService';

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

vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),


        transcribeFile: vi.fn()
    }
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn()
    }
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'history-1', title: 'Batch test.wav', projectId: null })
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
        useTranscriptStore.getState().setAudioUrl(null);
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.setState({
            sourceHistoryId: null,
            title: null,
            icon: null,
        });
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                streamingModelPath: "/path/to/model",
                offlineModelPath: '/path/to/model',
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
        );

        // Assert Item Status
        const queueState = useBatchQueueStore.getState();
        expect(queueState.queueItems[0].status).toBe('complete');
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
});
