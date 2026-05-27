import { describe, it, expect } from 'vitest';
import { splitByPunctuation, performSegmentSplit } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('splitByPunctuation', () => {
    it('correctly identifies start time when text has extra spaces BEFORE punctuation (Small Drift)', () => {
        const segment: TranscriptSegment = {
            id: '1',
            text: 'Hello    . World. Next', // Spaces before dot
            start: 0,
            end: 3.0,
            isFinal: true,
            tokens: ['Hello', '.', 'World', '.', 'Next'],
            timestamps: [0.0, 0.5, 1.0, 1.5, 2.0]
        };
        // "Hello" (5) + "." (1) = 6 tokens.
        // "Hello    " (9) + "." (1) = 10 text.
        // charIndex 10.
        // effectiveIndex: "Hello    .".replace spaces -> "Hello." -> 6.
        // Token "World" starts at currentLen 6.
        // 6 >= 6 && 6 < 11. Matches "World". Correct.

        const result = splitByPunctuation([segment]);

        expect(result[1].text.trim()).toBe('World.');
        expect(result[1].start).toBe(1.0);
    });

    it('correctly identifies start time with Large Drift (many words)', () => {
        // Text: "A B C D E F G H I J". (19 chars)
        // Tokens: ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"]. (Length 1 each)
        // Timestamps: 0, 1, 2, 3, 4, 5, 6, 7, 8, 9.

        // Punctuation split: "A B C D E. F G H I J"
        // Text: "A B C D E. F G H I J"
        // Tokens: ["A", "B", "C", "D", "E", ".", "F", "G", "H", "I", "J"]
        // Timestamps: 0, 1, 2, 3, 4, 4.5, 5, 6, 7, 8, 9

        // Target: "F" (start of next sentence).
        // Text index of "F".
        // "A B C D E. " = 11 chars.
        // "F" is at index 11.

        // Token index of "F".
        // A, B, C, D, E, . (6 tokens). Length 1 each.
        // "F" is at token index 6.

        // Old Logic (without effective index):
        // charIndex 11.
        // Token "F" starts at 6.
        // `6 >= 11` False.
        // Token "J" starts at 10. `10 >= 11` False.
        // It skips EVERYTHING until token length sum catches up?
        // Or if tokens are short, it never catches up?
        // It skips all tokens.

        // My previous fix (`currentLen + len > charIndex`):
        // Token "F" (start 6, end 7).
        // `7 > 11` False.
        // Skips "F".
        // Token "J" (start 10, end 11).
        // `11 > 11` False.
        // Skips "J".
        // Fails completely.

        // New Logic (Effective Index):
        // charIndex 11.
        // Text slice: "A B C D E. "
        // Replace spaces: "ABCDE." (Length 6).
        // effectiveIndex = 6.

        // Token "F" starts at currentLen 6.
        // `6 >= 6 && 6 < 7`. True.
        // Matches "F" (timestamp 5). Correct.

        const segment: TranscriptSegment = {
            id: '2',
            text: 'A B C D E. F G H I J',
            start: 0,
            end: 10.0,
            isFinal: true,
            tokens: ['A', 'B', 'C', 'D', 'E', '.', 'F', 'G', 'H', 'I', 'J'],
            timestamps: [0, 1, 2, 3, 4, 4.5, 5, 6, 7, 8, 9]
        };

        const result = splitByPunctuation([segment]);

        expect(result).toHaveLength(2);
        expect(result[1].text.trim()).toBe('F G H I J');
        expect(result[1].start).toBe(5.0);
    });

    it('correctly identifies start time when text has heavy punctuation but tokens do not', () => {
        // This reproduces the issue where text has punctuation (added by post-processing)
        // but tokens (from ASR) do not.
        // If we count punctuation in text index but not in token length,
        // the index drifts forward and misses the correct token.

        // Expected split:
        // 1. "Hello!!!!!!" (approx end?)
        // 2. "World." (Start time MUST be 1.0, matching "World" token)

        // Current buggy behavior:
        // "Hello!!!!!!" -> 11 chars.
        // "Hello" token -> 5 chars.
        // Index 11 is past "Hello" (0-5) and "World" (5-10).
        // So it likely returns undefined or next token if it existed.
        // If there was a third token "Peace" at 2.0:
        // Index 11 matches "Peace" (10-15).
        // So "World" would start at 2.0 (Later than actual).

        // Let's add a third token to demonstrate the "later than actual" shift clearly
        const segmentShift: TranscriptSegment = {
            id: 'repro-2',
            text: 'Hello!!!!!! World. Peace.',
            start: 0,
            end: 3.0,
            isFinal: true,
            tokens: ['Hello', 'World', 'Peace'],
            timestamps: [0.0, 1.0, 2.0]
        };

        const result = splitByPunctuation([segmentShift]);

        // We expect "World" to be the second segment
        const worldSegment = result.find(s => s.text.includes('World'));

        expect(worldSegment).toBeDefined();
        // With the bug, this might be 2.0 or interpolated.
        // We want it to be exactly 1.0 (start of "World" token).
        expect(worldSegment!.start).toBe(1.0);
    });

    it('correctly handles non-zero segment start time (absolute timestamp conversion)', () => {
        // Test ensures that relative token timestamps are converted to absolute
        const segment: TranscriptSegment = {
            id: 'offset-1',
            text: 'Hello. World.',
            start: 100.0,
            end: 105.0,
            isFinal: true,
            tokens: ['Hello', '.', 'World', '.'],
            // Absolute timestamps: 100.0, 100.5, 102.0, 102.5
            timestamps: [100.0, 100.5, 102.0, 102.5]
        };

        const result = splitByPunctuation([segment]);

        expect(result).toHaveLength(2);

        // First segment "Hello."
        // Start should be token[0] + segment.start = 0 + 100 = 100
        expect(result[0].text.trim()).toBe('Hello.');
        expect(result[0].start).toBe(100.0);

        // Second segment "World."
        // Start should be token[2] ("World") + segment.start = 2.0 + 100 = 102.0
        expect(result[1].text.trim()).toBe('World.');
        expect(result[1].start).toBe(102.0);
    });
});

describe('performSegmentSplit', () => {

    it('correctly splits a segment with modern timing units', () => {
        const segment: TranscriptSegment = {
            id: 'seg-1',
            start: 10.0,
            end: 20.0,
            text: 'Hello World',
            isFinal: true,
            timing: {
                units: [
                    { text: 'Hello', start: 10.0, end: 12.0 },
                    { text: ' ', start: 12.0, end: 13.0 },
                    { text: 'World', start: 13.0, end: 15.0 },
                ],
                level: 'token' as const,
                source: 'model' as const
            }
        };

        const { segmentLeft, segmentRight } = performSegmentSplit(
            segment,
            6, // caretOffset after 'Hello ' (length 6)
            'Hello World',
            'Hello',
            'World',
            'new-id-123'
        );

        expect(segmentLeft.end).toBe(13.0); // start of 'World' unit
        expect(segmentLeft.text).toBe('Hello');
        expect(segmentLeft.timing?.units).toHaveLength(2); // 'Hello' and ' '

        expect(segmentRight.id).toBe('new-id-123');
        expect(segmentRight.start).toBe(13.0);
        expect(segmentRight.end).toBe(20.0);
        expect(segmentRight.text).toBe('World');
        expect(segmentRight.timing?.units).toHaveLength(1); // 'World'
    });

    it('correctly splits a segment with legacy tokens and timestamps', () => {
        const segment: TranscriptSegment = {
            id: 'seg-2',
            start: 0,
            end: 5.0,
            text: 'LeftRight',
            isFinal: true,
            tokens: ['Left', 'Right'],
            timestamps: [1.0, 3.0],
            durations: [1.0, 1.0]
        };

        const { segmentLeft, segmentRight } = performSegmentSplit(
            segment,
            4, // caretOffset after 'Left' (length 4)
            'LeftRight',
            'Left',
            'Right',
            'new-id-456'
        );

        expect(segmentLeft.end).toBe(3.0); // timestamp of 'Right'
        expect(segmentLeft.tokens).toEqual(['Left']);
        expect(segmentLeft.timestamps).toEqual([1.0]);
        expect(segmentLeft.durations).toEqual([1.0]);

        expect(segmentRight.start).toBe(3.0);
        expect(segmentRight.tokens).toEqual(['Right']);
        expect(segmentRight.timestamps).toEqual([3.0]);
        expect(segmentRight.durations).toEqual([1.0]);
    });

    it('falls back to proportional timing when no timing information is available', () => {
        const segment: TranscriptSegment = {
            id: 'seg-3',
            start: 10.0,
            end: 20.0,
            text: 'abcdefghij',
            isFinal: true
        };

        const { segmentLeft, segmentRight } = performSegmentSplit(
            segment,
            3, // caretOffset
            'abcdefghij',
            'abc',
            'defghij',
            'new-id-789'
        );

        expect(segmentLeft.end).toBe(13.0); // 10.0 + 0.3 * 10
        expect(segmentRight.start).toBe(13.0);
    });
});
