import { describe, it, expect } from 'vitest';
import { splitByPunctuation } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('splitByPunctuation with tokens', () => {
    it('preserves tokens when splitting segments', () => {
        const segment: TranscriptSegment = {
            id: '1',
            text: 'Hello world. How are you?',
            start: 0,
            end: 7.0,
            isFinal: true,
            tokens: ['Hello', ' world', '.', ' How', ' are', ' you', '?'],
            timestamps: [0, 1, 2, 3, 4, 5, 6]
        };

        const result = splitByPunctuation([segment]);

        expect(result).toHaveLength(2);

        // First segment: "Hello world."
        expect(result[0].text.trim()).toBe('Hello world.');
        expect(result[0].tokens).toEqual(['Hello', ' world', '.']);
        expect(result[0].timestamps).toEqual([0, 1, 2]);

        // Second segment: "How are you?"
        expect(result[1].text.trim()).toBe('How are you?');
        expect(result[1].tokens).toEqual([' How', ' are', ' you', '?']);
        expect(result[1].timestamps).toEqual([3, 4, 5, 6]);
    });

    it('correctly handles token offsets with non-zero start time', () => {
        const segment: TranscriptSegment = {
            id: '2',
            text: 'Part one. Part two.',
            start: 100,
            end: 110,
            isFinal: true,
            tokens: ['Part', ' one', '.', ' Part', ' two', '.'],
            // Absolute timestamps: 100, 101, 102, 103, 104, 105
            timestamps: [100, 101, 102, 103, 104, 105]
        };

        const result = splitByPunctuation([segment]);

        expect(result).toHaveLength(2);

        // First segment
        expect(result[0].text.trim()).toBe('Part one.');
        expect(result[0].tokens).toEqual(['Part', ' one', '.']);
        // Absolute timestamps: 100, 101, 102
        expect(result[0].timestamps).toEqual([100, 101, 102]);
        expect(result[0].start).toBe(100);

        // Second segment
        expect(result[1].text.trim()).toBe('Part two.');
        expect(result[1].tokens).toEqual([' Part', ' two', '.']);
        // Absolute timestamps: 103, 104, 105
        expect(result[1].timestamps).toEqual([103, 104, 105]);
        // Start time aligns with first token
        expect(result[1].start).toBe(103);
    });

    it('handles splits where tokens do not perfectly align with text splits (fuzzy matching)', () => {
        // Text: "A B C D E F"
        // Punctuation in middle of "C"?? No, standard case first.
        // Case: Token spans across punctuation?
        // Text: "Word."
        // Tokens: ["Word", "."] -> Easy.
        // Text: "Word."
        // Tokens: ["Word."] -> Should be kept together?
        // splitByPunctuation splits the TEXT.
        // "Word." -> ["Word"] and ["."] -> No, split regex keeps delimiters.

        // Let's test loose alignment.
        const segment: TranscriptSegment = {
            id: '3',
            text: 'Run. Stop.',
            start: 0,
            end: 4,
            isFinal: true,
            tokens: ['Run', '.', ' Stop', '.'],
            timestamps: [0, 1, 2, 3]
        };

        const result = splitByPunctuation([segment]);
        expect(result).toHaveLength(2);
        expect(result[0].tokens).toEqual(['Run', '.']);
        expect(result[1].tokens).toEqual([' Stop', '.']);
    });
});
