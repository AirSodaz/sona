import { describe, it, expect } from 'vitest';
import { splitByPunctuation } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('splitByPunctuation Advanced Tests', () => {
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

        expect(result).toHaveLength(1);
        expect(result[0].text).toBe('Dr. Smith met Mr. Jones at the clinic.');
    });

    it('should split long Western segments (> 84 chars) on commas', () => {
        const text = 'The quick brown fox jumps over the lazy dog, and the dog was not amused, because it was sleeping peacefully in the sun.';
        // Length is ~110 chars.

        const segment: TranscriptSegment = {
            id: 'long-western-1',
            text: text,
            start: 0,
            end: 10.0,
            isFinal: true,
            tokens: text.split(' '),
            timestamps: text.split('').map((_, i) => i * 0.1)
        };

        const result = splitByPunctuation([segment]);

        expect(result.length).toBeGreaterThan(1);

        // Verify no segment is excessively long (using the new limit of 84)
        result.forEach(seg => {
            expect(seg.text.length).toBeLessThan(85);
        });
    });

    it('should NOT split Western segments under 84 chars', () => {
        const text = 'The quick brown fox jumps over the lazy dog, and the dog was barely amused.';
        // Length is ~75 chars.

        const segment: TranscriptSegment = {
            id: 'short-western-1',
            text: text,
            start: 0,
            end: 5.0,
            isFinal: true,
            tokens: text.split(' '),
            timestamps: text.split('').map((_, i) => i * 0.1)
        };

        const result = splitByPunctuation([segment]);

        expect(result).toHaveLength(1);
        expect(result[0].text).toBe(text);
    });

    it('should split long CJK segments (> 36 chars) on commas', () => {
        // A long Chinese sentence.
        const text = '这是一个非常长的中文句子，主要用于测试分段逻辑是否正确，因为它包含了逗号，所以应该被分割成更小的部分，以确保字幕的可读性。';
        // Length is ~60 chars.

        const segment: TranscriptSegment = {
            id: 'long-cjk-1',
            text: text,
            start: 0,
            end: 10.0,
            isFinal: true,
            tokens: text.split(''),
            timestamps: text.split('').map((_, i) => i * 0.1)
        };

        const result = splitByPunctuation([segment]);

        expect(result.length).toBeGreaterThan(1);

        // Verify no segment is excessively long (using the new limit of 36)
        result.forEach(seg => {
            expect(seg.text.length).toBeLessThan(37);
        });
    });
});
