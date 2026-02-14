import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore } from '../transcriptStore';
import { transcriptionService } from '../../services/transcriptionService';

// Mock dependencies
vi.mock('uuid', () => ({
    v4: () => 'test-uuid'
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

describe('batchQueueStore buffering', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        useBatchQueueStore.getState().clearQueue();
        useTranscriptStore.getState().setConfig({ offlineModelPath: '/model' });
        vi.clearAllMocks();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('should buffer segments and flush periodically', async () => {
        const file = '/test.wav';
        const totalSegments = 60;

        // Setup mock to simulate streaming
        vi.mocked(transcriptionService.transcribeFile).mockImplementation(
            async (_filePath, _onProgress, onSegment) => {
                if (!onSegment) return [];

                // Emit 60 segments
                for (let i = 0; i < totalSegments; i++) {
                    onSegment({
                        id: `seg-${i}`,
                        text: `Segment ${i}`,
                        start: i,
                        end: i + 1,
                        isFinal: true
                    });

                    // Wait 10ms (virtual time)
                    await new Promise(resolve => setTimeout(resolve, 10));
                }

                // Return full list as final result
                const finalSegments = Array.from({ length: totalSegments }).map((_, i) => ({
                    id: `seg-${i}`,
                    text: `Segment ${i}`,
                    start: i,
                    end: i + 1,
                    isFinal: true
                }));
                return finalSegments;
            }
        );

        let updateCount = 0;
        const unsub = useBatchQueueStore.subscribe((state, prevState) => {
            // We want to count updates to segments specifically
            const prevSegs = prevState.queueItems[0]?.segments;
            const currSegs = state.queueItems[0]?.segments;

            // Only count if segments reference changed and length increased
            // (To filter out initial empty state or other updates)
            if (currSegs && prevSegs && currSegs !== prevSegs) {
                updateCount++;
            }
        });

        // Action
        useBatchQueueStore.getState().addFiles([file]);

        // Wait for async processing
        await vi.runAllTimersAsync();

        unsub();

        // 60 segments.
        // Thresholds: 50 items OR 500ms.
        // Loop runs for 60 iterations. 10ms each. Total 600ms.

        // Buffer flush condition: `segmentBuffer.length >= 50 || now - lastUpdateTime > 500`

        // i=0..49 (50 items).
        // At i=49 (count 50). FLUSH.
        // i=50..59 (10 items). Time from last flush: 0..100ms.
        // Loop ends.
        // transcribeFile returns.
        // Final update.

        // Total updates: 2.
        // Without buffering: 60.

        console.log('Update count:', updateCount);
        expect(updateCount).toBeGreaterThan(0);
        expect(updateCount).toBeLessThan(10);

        const item = useBatchQueueStore.getState().queueItems[0];
        expect(item.segments.length).toBe(60);
    });
});
