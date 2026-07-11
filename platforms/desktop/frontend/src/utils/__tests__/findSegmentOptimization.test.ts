import { describe, it, expect } from 'vitest';
import { findSegmentAndIndexForTime } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

const createSegment = (id: string, start: number, end: number): TranscriptSegment => ({
    id,
    text: 'text',
    start,
    end,
    isFinal: true
});

describe('findSegmentAndIndexForTime Optimization', () => {
    const segments = [
        createSegment('1', 10, 20),
        createSegment('2', 30, 40),
        createSegment('3', 50, 60)
    ];

    it('returns segment and index when inside a segment', () => {
        const result = findSegmentAndIndexForTime(segments, 15);
        expect(result.segment?.id).toBe('1');
        expect(result.index).toBe(0);
    });

    // Current behavior: returns -1. Optimization goal: return 0.
    it('returns undefined segment but closest index when in a gap (Forward Playback)', () => {
        // Gap between 1 and 2 (20-30). Time 25.
        // Should return index 0 (segment 1) as the preceding segment.
        const result = findSegmentAndIndexForTime(segments, 25);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(0);
    });

    it('uses hint to avoid binary search in gap (Forward)', () => {
        // Gap after segment 1. Hint 0.
        // Time 25.
        const result = findSegmentAndIndexForTime(segments, 25, 0);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(0);
    });

    it('uses hint to avoid binary search in gap (Rewind)', () => {
        // Gap between 1 and 2. Time 25.
        // Hint 1 (we were in segment 2).
        const result = findSegmentAndIndexForTime(segments, 25, 1);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(0); // Should return preceding segment index
    });

    it('handles pre-roll silence (before first segment)', () => {
        const result = findSegmentAndIndexForTime(segments, 5);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });

    it('uses hint -1 for pre-roll optimization', () => {
        const result = findSegmentAndIndexForTime(segments, 5, -1);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });
});
