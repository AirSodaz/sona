import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore } from '../transcriptStore';

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
        setPunctuationModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
        setSourceFilePath: vi.fn(), // Added mock
        transcribeFile: vi.fn()
    }
}));

vi.mock('../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn()
    }
}));

describe('batchQueueStore', () => {
    beforeEach(() => {
        useBatchQueueStore.getState().clearQueue();
        useTranscriptStore.getState().setAudioUrl(null);
        useTranscriptStore.getState().clearSegments();
        vi.clearAllMocks();
    });

    it('should automatically set active item and sync to transcript store when adding files', () => {
        const files = ['/path/to/test.wav'];

        // Action
        useBatchQueueStore.getState().addFiles(files);

        // Assert Queue State
        const queueState = useBatchQueueStore.getState();
        expect(queueState.queueItems).toHaveLength(1);
        expect(queueState.activeItemId).toBe('test-uuid-123');

        // Assert Transcript Store State (The Bug)
        // This is expected to fail currently because addFiles updates local state 
        // but doesn't sync to transcriptStore
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBe('asset:///path/to/test.wav');
    });

    it('should sync to transcript store when removing the active item', () => {
        // Setup
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav' },
                { id: '2', filename: '2.wav', filePath: '/2.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///2.wav' }
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

        // Assert Transcript Store State (The Bug)
        const transcriptState = useTranscriptStore.getState();
        expect(transcriptState.audioUrl).toBe('asset:///2.wav');
    });

    it('should NOT sync to transcript store when removing a non-active item', () => {
        // Setup
        useBatchQueueStore.setState({
            queueItems: [
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav' },
                { id: '2', filename: '2.wav', filePath: '/2.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///2.wav' }
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
                { id: '1', filename: '1.wav', filePath: '/1.wav', status: 'pending', progress: 0, segments: [], audioUrl: 'asset:///1.wav' }
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
