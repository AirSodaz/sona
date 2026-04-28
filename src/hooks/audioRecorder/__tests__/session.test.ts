import { describe, expect, it, vi } from 'vitest';
import { createRecordSessionController } from '../session';
import type { MutableRefLike, RecordSessionPhase, RecordSessionRefs } from '../types';

function createRef<T>(value: T): MutableRefLike<T> {
    return { current: value };
}

describe('createRecordSessionController', () => {
    it('ignores stale resets from an older session', () => {
        const refs: RecordSessionRefs = {
            recordSessionIdRef: createRef<string | null>(null),
            recordSessionPhaseRef: createRef<RecordSessionPhase>('idle'),
            peakLevelRef: createRef(0),
        };
        const resetLiveTimingState = vi.fn();
        const clearFinalizedDurationSeconds = vi.fn();
        const clearSegments = vi.fn();
        const setIsRecording = vi.fn();
        const setIsPaused = vi.fn();
        const setIsTransitioning = vi.fn();

        const session = createRecordSessionController({
            refs,
            logger: {
                info: vi.fn(),
                warn: vi.fn(),
                error: vi.fn(),
            },
            clearSegments,
            resetLiveTimingState,
            clearFinalizedDurationSeconds,
            beginRecordedDurationWindow: vi.fn(),
            syncRecordingElapsedMs: vi.fn(),
            setIsRecording,
            setIsPaused,
            setIsTransitioning,
            getInputSource: () => 'microphone',
            getIsRecording: () => false,
            softStopRecordRuntime: vi.fn().mockResolvedValue(undefined),
        });

        const firstSessionId = session.openRecordSession();
        const secondSessionId = session.openRecordSession();

        session.resetRecordSession(firstSessionId, 'stale_reset', true);

        expect(session.getSessionId()).toBe(secondSessionId);
        expect(refs.recordSessionPhaseRef.current).toBe('starting');
        expect(clearSegments).toHaveBeenCalledTimes(2);
        expect(setIsRecording).toHaveBeenCalledTimes(2);
        expect(setIsPaused).toHaveBeenCalledTimes(2);
        expect(setIsTransitioning).not.toHaveBeenCalled();
        expect(resetLiveTimingState).toHaveBeenCalledTimes(2);
        expect(clearFinalizedDurationSeconds).toHaveBeenCalledTimes(2);
    });
});
