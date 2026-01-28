import { describe, it, expect, beforeEach } from 'vitest';
import { useTranscriptStore } from '../transcriptStore';
import { TranscriptSegment } from '../../types/transcript';
import { v4 as uuidv4 } from 'uuid';

describe('TranscriptStore Performance', () => {
    beforeEach(() => {
        useTranscriptStore.getState().clearSegments();
    });

    it('benchmarks upsertSegment for streaming updates', () => {
        const ITERATIONS = 1000;
        const INITIAL_SIZE = 10000;

        // Seed with initial segments
        const segments: TranscriptSegment[] = [];
        for (let i = 0; i < INITIAL_SIZE; i++) {
            segments.push({
                id: uuidv4(),
                text: `Segment ${i}`,
                start: i * 2,
                end: i * 2 + 1.5,
                isFinal: true
            });
        }
        useTranscriptStore.getState().setSegments(segments);

        // Scenario 1: Streaming update (updating the last segment repeatedly)
        const lastSegment = segments[segments.length - 1];

        console.time('streaming-update');
        for (let i = 0; i < ITERATIONS; i++) {
            useTranscriptStore.getState().upsertSegment({
                ...lastSegment,
                text: `Updated text ${i}`,
                end: lastSegment.end + (i * 0.1)
            });
        }
        console.timeEnd('streaming-update');

        // Verify state integrity
        const finalSegments = useTranscriptStore.getState().segments;
        expect(finalSegments.length).toBe(INITIAL_SIZE);
        expect(finalSegments[finalSegments.length - 1].text).toBe(`Updated text ${ITERATIONS - 1}`);
    });

    it('benchmarks upsertSegment for streaming append', () => {
        const ITERATIONS = 1000;
        const INITIAL_SIZE = 10000;

        // Seed with initial segments
        const segments: TranscriptSegment[] = [];
        for (let i = 0; i < INITIAL_SIZE; i++) {
            segments.push({
                id: uuidv4(),
                text: `Segment ${i}`,
                start: i * 2,
                end: i * 2 + 1.5,
                isFinal: true
            });
        }
        useTranscriptStore.getState().setSegments(segments);

        // Scenario 2: Appending new segments sequentially
        let lastEndTime = segments[segments.length - 1].end;

        console.time('streaming-append');
        for (let i = 0; i < ITERATIONS; i++) {
            const newSegment: TranscriptSegment = {
                id: uuidv4(),
                text: `New Segment ${i}`,
                start: lastEndTime + 0.1,
                end: lastEndTime + 1.0,
                isFinal: true
            };
            useTranscriptStore.getState().upsertSegment(newSegment);
            lastEndTime = newSegment.end;
        }
        console.timeEnd('streaming-append');

        const finalSegments = useTranscriptStore.getState().segments;
        expect(finalSegments.length).toBe(INITIAL_SIZE + ITERATIONS);
    });
});
