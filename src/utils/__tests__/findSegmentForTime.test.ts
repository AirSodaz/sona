import { describe, it, expect } from 'vitest';
import { findSegmentForTime } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('findSegmentForTime', () => {
    const segments: TranscriptSegment[] = [
        { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
        { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
        { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
        { id: '4', start: 4, end: 5, text: 'Four', isFinal: true, tokens: [], timestamps: [] }, // Gap between 3 and 4
    ];

    it('finds segment using binary search (no hint)', () => {
        const result = findSegmentForTime(segments, 0.5);
        expect(result.segment).toBeDefined();
        expect(result.segment?.id).toBe('1');
        expect(result.index).toBe(0);
    });

    it('finds segment using correct hint (O(1))', () => {
        // Mock access to prove O(1)? Hard in unit test, but we verify correctness
        const result = findSegmentForTime(segments, 1.5, 1);
        expect(result.segment?.id).toBe('2');
        expect(result.index).toBe(1);
    });

    it('finds next segment using hint (O(1) sequential)', () => {
        // Hint points to '1', but time is in '2'
        const result = findSegmentForTime(segments, 1.5, 0);
        expect(result.segment?.id).toBe('2');
        expect(result.index).toBe(1);
    });

    it('falls back to binary search if hint is far off', () => {
        // Hint points to '1', but time is in '4'
        const result = findSegmentForTime(segments, 4.5, 0);
        expect(result.segment?.id).toBe('4');
        expect(result.index).toBe(3);
    });

    it('returns undefined/index -1 if time is in a gap', () => {
        const result = findSegmentForTime(segments, 3.5);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });

    it('returns undefined/index -1 if time is before first segment', () => {
        const result = findSegmentForTime(segments, -1);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });

    it('returns undefined/index -1 if time is after last segment', () => {
        const result = findSegmentForTime(segments, 6);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });

    it('handles empty segments array', () => {
        const result = findSegmentForTime([], 1);
        expect(result.segment).toBeUndefined();
        expect(result.index).toBe(-1);
    });
});
