import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore } from '../transcriptStore';
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
        setPunctuationModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
        setSourceFilePath: vi.fn(),
        setCtcModelPath: vi.fn(),
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
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'history-1' })
    }
}));

describe('batchQueueStore History Integration', () => {
    beforeEach(() => {
        useBatchQueueStore.getState().clearQueue();
        useTranscriptStore.getState().setAudioUrl(null);
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.setState({
            config: {
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
        useTranscriptStore.setState({
            config: {
                ...useTranscriptStore.getState().config,
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
            '/tmp/test-uuid-123.wav'
        );

        // Assert Item Status
        const queueState = useBatchQueueStore.getState();
        expect(queueState.queueItems[0].status).toBe('complete');
    });
});
