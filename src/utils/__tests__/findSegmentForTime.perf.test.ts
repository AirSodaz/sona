import { describe, it, expect, bench } from 'vitest';
import { findSegmentForTime, findSegmentAndIndexForTime } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';
import { v4 as uuidv4 } from 'uuid';

// Helper to generate segments
function generateSegments(count: number): TranscriptSegment[] {
    const segments: TranscriptSegment[] = [];
    let currentTime = 0;
    for (let i = 0; i < count; i++) {
        const duration = 2 + Math.random() * 3; // 2-5 seconds
        segments.push({
            id: uuidv4(),
            text: `Segment ${i}`,
            start: currentTime,
            end: currentTime + duration,
            isFinal: true,
        });
        currentTime += duration;
    }
    return segments;
}

describe('findSegmentForTime Performance', () => {
    const segments = generateSegments(10000); // 10k segments
    const totalDuration = segments[segments.length - 1].end;

    // Simulate playback: query every 100ms
    const queries: number[] = [];
    for (let t = 0; t < totalDuration; t += 0.1) {
        queries.push(t);
    }

    it('benchmarks linear playback queries (without hint)', () => {
        const start = performance.now();
        let hits = 0;

        // This simulates usage without hint (binary search)
        for (const time of queries) {
            const segment = findSegmentForTime(segments, time);
            if (segment) hits++;
        }

        const end = performance.now();
        console.log(`Without Hint: Querying ${queries.length} timepoints on ${segments.length} segments took ${(end - start).toFixed(2)}ms`);

        expect(hits).toBeGreaterThan(0);
    });

    it('benchmarks linear playback queries (with hint)', () => {
        const start = performance.now();
        let hits = 0;
        let lastIndex = -1;

        // This simulates usage with hint
        for (const time of queries) {
            const { segment, index } = findSegmentAndIndexForTime(segments, time, lastIndex);
            if (segment) hits++;
            lastIndex = index;
        }

        const end = performance.now();
        console.log(`With Hint:    Querying ${queries.length} timepoints on ${segments.length} segments took ${(end - start).toFixed(2)}ms`);

        expect(hits).toBeGreaterThan(0);
    });
});
