import { describe, it, expect, beforeEach } from 'vitest';
import { useTranscriptStore } from '../transcriptStore';
import { TranscriptSegment } from '../../types/transcript';
import { v4 as uuidv4 } from 'uuid';

describe('TranscriptStore', () => {
    beforeEach(() => {
        useTranscriptStore.getState().clearSegments();
    });

    describe('setActiveSegmentId', () => {
        it('should reset activeSegmentIndex to -1 when index is not provided', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 1, isFinal: true },
                { id: id2, text: 'Second', start: 1, end: 2, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Set active segment without index
            useTranscriptStore.getState().setActiveSegmentId(id2);

            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(-1);
        });

        it('should set activeSegmentIndex when index is provided', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 1, isFinal: true },
                { id: id2, text: 'Second', start: 1, end: 2, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Set active segment with index
            // @ts-ignore: Argument of type 'number' is not assignable to parameter of type 'never' (yet)
            useTranscriptStore.getState().setActiveSegmentId(id2, 1);

            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(1);
        });
    });

    describe('requestSeek', () => {
        it('should update currentTime and set seekRequest', () => {
            const time = 10.5;
            useTranscriptStore.getState().requestSeek(time);

            expect(useTranscriptStore.getState().currentTime).toBe(time);
            expect(useTranscriptStore.getState().seekRequest).toEqual({
                time,
                timestamp: expect.any(Number)
            });
            expect(useTranscriptStore.getState().lastSeekTimestamp).toEqual(expect.any(Number));
        });

        it('should update activeSegmentId based on seek time', () => {
            const id1 = uuidv4();
            const id2 = uuidv4();
            const segments: TranscriptSegment[] = [
                { id: id1, text: 'First', start: 0, end: 5, isFinal: true },
                { id: id2, text: 'Second', start: 5, end: 10, isFinal: true }
            ];

            useTranscriptStore.getState().setSegments(segments);

            // Seek to middle of second segment
            useTranscriptStore.getState().requestSeek(7.5);

            expect(useTranscriptStore.getState().currentTime).toBe(7.5);
            expect(useTranscriptStore.getState().activeSegmentId).toBe(id2);
            // activeSegmentIndex should be set by findSegmentAndIndexForTime
            expect(useTranscriptStore.getState().activeSegmentIndex).toBe(1);
        });
    });
});
