import { describe, it, expect } from 'vitest';
import { splitByPunctuation } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('splitByPunctuation Reproduction Tests', () => {
    it('should not split on common abbreviations like "Mr.", "Dr.", etc.', () => {
        const segment: TranscriptSegment = {
            id: 'abbr-1',
            text: 'Dr. Smith met Mr. Jones at the clinic.',
            start: 0,
            end: 5.0,
            isFinal: true,
            tokens: ['Dr', '.', 'Smith', 'met', 'Mr', '.', 'Jones', 'at', 'the', 'clinic', '.'],
            timestamps: [0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]
        };

        const result = splitByPunctuation([segment]);

        // Current behavior (FAIL): Splits into "Dr.", " Smith met Mr.", " Jones at the clinic."
        // Desired behavior (PASS): Keeps as one segment (or splits only at the very end if there was a sentence boundary)

        // If it splits correctly, result length should be 1
        expect(result).toHaveLength(1);
        expect(result[0].text).toBe('Dr. Smith met Mr. Jones at the clinic.');
    });

    it('should split long segments (> 100 chars) on commas', () => {
        const text = 'The quick brown fox jumps over the lazy dog, and the dog was not amused, because it was sleeping peacefully in the sun, dreaming of chasing rabbits in the field.';
        // Length is ~150 chars.

        const segment: TranscriptSegment = {
            id: 'long-1',
            text: text,
            start: 0,
            end: 10.0,
            isFinal: true,
            // Mock tokens roughly (not critical for logic unless we validate strict timestamps)
            tokens: text.split(' '),
            timestamps: text.split('').map((_, i) => i * 0.1) // Dummy timestamps
        };

        const result = splitByPunctuation([segment]);

        // Current behavior (FAIL): Returns 1 segment because no strong punctuation.
        // Desired behavior (PASS): Returns multiple segments (split by commas).

        expect(result.length).toBeGreaterThan(1);

        // Verify no segment is excessively long
        result.forEach(seg => {
            expect(seg.text.length).toBeLessThan(100);
        });
    });
});
