import { describe, expect, it, vi } from 'vitest';
import { createRecordTimingController } from '../timing';
import type { MutableRefLike, RecordTimingRefs } from '../types';

function createRef<T>(value: T): MutableRefLike<T> {
    return { current: value };
}

describe('createRecordTimingController', () => {
    it('accumulates duration across pause and resume and advances the next offset', () => {
        let now = 0;
        const setRecordingElapsedMs = vi.fn();
        const refs: RecordTimingRefs = {
            recordedDurationMsRef: createRef(0),
            activeDurationStartedAtRef: createRef<number | null>(null),
            finalizedDurationSecondsRef: createRef<number | null>(null),
            segmentTimeOffsetSecondsRef: createRef(0),
            recordTimelineCursorSecondsRef: createRef(0),
        };

        const timing = createRecordTimingController({
            refs,
            logger: {
                info: vi.fn(),
                warn: vi.fn(),
                error: vi.fn(),
            },
            setRecordingElapsedMs,
            getSessionId: () => 'record-session',
            now: () => now,
        });

        timing.beginRecordedDurationWindow();
        now = 2000;
        expect(timing.getRecordedDurationMs()).toBe(2000);

        timing.pauseRecordedDurationWindow();
        expect(timing.getRecordedDurationSeconds()).toBe(2);
        expect(timing.getNextSegmentTimeOffsetSeconds()).toBe(2);

        timing.trackAcceptedSegment({ id: 'seg-1', text: 'hello', start: 0, end: 2.5, isFinal: true });
        expect(timing.getNextSegmentTimeOffsetSeconds()).toBe(2.5);

        now = 7000;
        timing.beginRecordedDurationWindow();
        now = 8000;
        expect(timing.getRecordedDurationMs()).toBe(3000);
        expect(timing.getNextSegmentTimeOffsetSeconds()).toBe(3);

        const finalized = timing.finalizeRecordedDurationSeconds();
        expect(finalized).toBe(3);
        expect(refs.finalizedDurationSecondsRef.current).toBe(3);
        expect(setRecordingElapsedMs).toHaveBeenLastCalledWith(3000);
    });
});
