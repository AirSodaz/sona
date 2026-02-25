import { stripHtmlTags, alignTokensToText } from '../segmentUtils';
import { describe, it, expect } from 'vitest';

describe('segmentUtils formatted text', () => {
    it('strips html tags', () => {
        expect(stripHtmlTags('<b>Hello</b>')).toBe('Hello');
        expect(stripHtmlTags('A <b>B</b> C')).toBe('A B C');
    });

    it('aligns tokens with formatting', () => {
        const text = '<b>Hello</b> world';
        const rawTokens = ['hello', 'world'];
        const rawTimestamps = [1.0, 2.0];

        const result = alignTokensToText(text, rawTokens, rawTimestamps);

        // Expected tokens:
        // 1. <b>Hello</b> (timestamp 1.0)
        // 2. " " (timestamp 1.0 or 2.0 - whitespace usually takes prev or next timestamp)
        // 3. world (timestamp 2.0)

        // Filter out whitespace for clearer assertion of content alignment
        const contentTokens = result.filter(t => t.text.trim().length > 0);

        expect(contentTokens).toHaveLength(2);
        expect(contentTokens[0].text).toBe('<b>Hello</b>');
        expect(contentTokens[0].timestamp).toBe(1.0);

        expect(contentTokens[1].text).toBe('world');
        expect(contentTokens[1].timestamp).toBe(2.0);
    });

    it('handles nested formatting', () => {
        const text = '<b><i>Hello</i></b>';
        const rawTokens = ['hello'];
        const rawTimestamps = [1.0];

        const result = alignTokensToText(text, rawTokens, rawTimestamps);
        const contentTokens = result.filter(t => t.text.trim().length > 0);

        expect(contentTokens).toHaveLength(1);
        // Canonical order of tags is alphabetical in my implementation?
        // Array.from(activeTags).sort()
        // b, i -> <b><i>Hello</i></b>
        expect(contentTokens[0].text).toBe('<b><i>Hello</i></b>');
    });

    it('handles punctuation merging with formatting', () => {
        const text = '<b>Hello</b>.';
        const rawTokens = ['hello']; // Punctuation often not in raw tokens or ignored
        const rawTimestamps = [1.0];

        const result = alignTokensToText(text, rawTokens, rawTimestamps);
        const contentTokens = result.filter(t => t.text.trim().length > 0);

        expect(contentTokens).toHaveLength(1);
        // "Hello" + "." merged.
        // "Hello" is <b>Hello</b>
        // "." is . (no tags? or tags from context?)
        // In "<b>Hello</b>.", "." is outside tags.
        // So merged: <b>Hello</b>.
        expect(contentTokens[0].text).toBe('<b>Hello</b>.');
    });

    it('handles punctuation merging inside formatting', () => {
        const text = '<b>Hello.</b>';
        const rawTokens = ['hello'];
        const rawTimestamps = [1.0];

        const result = alignTokensToText(text, rawTokens, rawTimestamps);
        const contentTokens = result.filter(t => t.text.trim().length > 0);

        expect(contentTokens).toHaveLength(1);
        // Lexer splits: <b>Hello</b>, <b>.</b> (because . is punctuation char?)
        // Wait, Lexer regex `([^\s\p{sc=Han}<>]+)`.
        // "Hello." matches as one word "Hello." if it doesn't contain < > space.
        // So "Hello." is one token.
        // Wrapped: <b>Hello.</b>
        expect(contentTokens[0].text).toBe('<b>Hello.</b>');
    });
});
