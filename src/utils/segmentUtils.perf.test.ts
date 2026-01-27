import { describe, it, expect } from 'vitest';
import { splitByPunctuation } from './segmentUtils';
import { TranscriptSegment } from '../types/transcript';
import { v4 as uuidv4 } from 'uuid';

describe('splitByPunctuation Performance', () => {
    it('should be fast for large segments', () => {
        // Generate a large segment with 1000 sentences
        const sentenceCount = 1000;
        let text = '';
        const tokens: string[] = [];
        const timestamps: number[] = [];
        let currentTime = 0;

        for (let i = 0; i < sentenceCount; i++) {
            const sentence = "This is a test sentence number " + i;
            const sentenceTokens = sentence.split(' '); // simple tokenization

            text += sentence + ". ";

            for (const token of sentenceTokens) {
                tokens.push(token);
                timestamps.push(currentTime);
                currentTime += 0.5;
            }
            // Punctuation is in text but usually not in tokens/timestamps for some models,
            // or is separate token. Let's assume punctuation is NOT in tokens to match the "drift" logic in utils.
            // "This is a test sentence number 0. "
            // tokens: ["This", "is", "a", "test", "sentence", "number", "0"]
        }

        const segment: TranscriptSegment = {
            id: uuidv4(),
            start: 0,
            end: currentTime,
            text: text.trim(),
            isFinal: true,
            tokens,
            timestamps
        };

        const start = performance.now();
        const result = splitByPunctuation([segment]);
        const end = performance.now();

        const duration = end - start;
        console.log(`Processing ${sentenceCount} sentences took ${duration.toFixed(2)}ms`);

        // With O(N^2) behavior, this should be slow (e.g. > 1000ms or much more)
        // With O(N) behavior, it should be very fast (< 100ms)

        expect(result.length).toBeGreaterThan(0);
        // Expect roughly 1000 segments (one per sentence)
        expect(result.length).toBe(sentenceCount);
    });
});
