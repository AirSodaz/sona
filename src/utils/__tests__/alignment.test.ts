
import { describe, it, expect } from 'vitest';

/**
 * Aligns formatted text with raw tokens to assign timestamps to display words.
 */
export function alignTokensToText(
    text: string,
    rawTokens: string[],
    rawTimestamps: number[]
): { text: string; timestamp: number }[] {
    const result: { text: string; timestamp: number }[] = [];

    if (!text || !rawTokens || !rawTimestamps || rawTokens.length !== rawTimestamps.length) {
        return [{ text: text, timestamp: rawTimestamps?.[0] || 0 }];
    }

    // Normalizing
    // Strip only characters that are NOT letters (Unicode aware) or numbers.
    const normalize = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');

    // Tokenize text:
    // 1. Whitespace (kept to preserve spacing)
    // 2. Chinese characters (Han script) treated as individual words
    // 3. Everything else (English, numbers, punctuation) grouped until whitespace/Han/End
    const rawWords = text.match(/(\s+|[\p{sc=Han}]|[^\s\p{sc=Han}]+)/gu) || [];

    // Merge standalone punctuation into the previous word
    const words: string[] = [];
    for (const w of rawWords) {
        // Check if w is purely punctuation (and previous word exists and is not whitespace)
        if (words.length > 0 && /^[^\p{L}\p{N}]+$/u.test(w) && !/^\s+$/.test(w) && !/^\s+$/.test(words[words.length - 1])) {
            // Append to previous word
            words[words.length - 1] += w;
        } else {
            words.push(w);
        }
    }

    let currentRawIndex = 0;

    for (let i = 0; i < words.length; i++) {
        const word = words[i];
        if (!word.trim()) {
            result.push({ text: word, timestamp: rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)] });
            continue;
        }

        const normWord = normalize(word);
        if (!normWord) {
            result.push({ text: word, timestamp: rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)] });
            continue;
        }

        const startTimestamp = rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)];
        result.push({ text: word, timestamp: startTimestamp });

        // Try to match `normWord` against next N tokens.
        let accumulatedTokenStr = "";
        let tokensConsumed = 0;
        let foundMatch = false;

        for (let j = 0; j < 5 && (currentRawIndex + j) < rawTokens.length; j++) {
            const t = rawTokens[currentRawIndex + j];
            accumulatedTokenStr += normalize(t);
            tokensConsumed++;

            if (accumulatedTokenStr.startsWith(normWord) || normWord.startsWith(accumulatedTokenStr)) {
                if (accumulatedTokenStr.length >= normWord.length) {
                    currentRawIndex += tokensConsumed;
                    foundMatch = true;
                    break;
                }
            }
        }

        if (!foundMatch) {
            // Drastic mismatch (ITN).
            // Find the NEXT content word in `words`.
            let nextNorm = "";
            for (let nextIdx = i + 1; nextIdx < words.length; nextIdx++) {
                nextNorm = normalize(words[nextIdx]);
                if (nextNorm) break;
            }

            if (nextNorm) {
                // distinct next word
                // Scan ahead in tokens to find `nextNorm`.
                for (let k = 1; k < 10 && (currentRawIndex + k) < rawTokens.length; k++) {
                    const t = normalize(rawTokens[currentRawIndex + k]);
                    if (t && t.startsWith(nextNorm)) {
                        // Found next word at k offset.
                        // So current word consumes everything up to k.
                        currentRawIndex += k;
                        foundMatch = true;
                        break;
                    }
                }
            } else {
                // If there is no next word, we are at the end.
                // Consume all remaining tokens? Or just 1.
                // If "100" vs "one" "hundred" and correct end is consumed.
                // Just consume everything remaining?
                if (i === words.length - 1 || (i > words.length - 3 && !words.slice(i + 1).some(w => normalize(w)))) {
                    currentRawIndex = rawTokens.length;
                    foundMatch = true;
                }
            }

            if (!foundMatch) {
                // Fallback: Just consume 1 token.
                currentRawIndex++;
            }
        }

        if (currentRawIndex >= rawTokens.length) currentRawIndex = rawTokens.length;
    }

    return result;
}

describe('alignTokensToText', () => {
    it('aligns simple punctuation', () => {
        const text = "Hello, world!";
        const tokens = ["hello", " world"];
        const timestamps = [1.0, 2.0];

        const result = alignTokensToText(text, tokens, timestamps);

        expect(result[0].text).toBe("Hello,");
        expect(result[0].timestamp).toBe(1.0);

        expect(result[2].text).toBe("world!");
        expect(result[2].timestamp).toBe(2.0);
    });

    it('aligns ITN mismatch (numbers)', () => {
        const text = "I have 100 dollars.";
        const tokens = ["i", " have", " one", " hundred", " dollars"];
        const timestamps = [0.5, 1.0, 1.5, 1.7, 2.0];
        // "I" -> "i" (0.5)
        // "have" -> " have" (1.0)
        // "100" -> " one" (1.5) ... implicit consume " hundred"?
        // "dollars." -> " dollars" (2.0)

        const result = alignTokensToText(text, tokens, timestamps);

        const w100 = result.find(r => r.text === "100");
        expect(w100?.timestamp).toBe(1.5);

        const wDollars = result.find(r => r.text === "dollars.");
        expect(wDollars?.timestamp).toBe(2.0);
    });
    it('aligns Chinese characters', () => {
        const text = "你好，世界。";
        // Tokens might be single chars or words.
        const tokens = ["你", "好", "，", "世", "界", "。"];
        const timestamps = [1.0, 1.2, 1.4, 1.6, 1.8, 2.0];

        const result = alignTokensToText(text, tokens, timestamps);
        console.log('Chinese Result:', JSON.stringify(result, null, 2));
        expect(result.length).toBeGreaterThan(0);
        // If normalization strips chars, we might get empty results or misalignment
        const first = result.find(r => r.text.includes("你好"));
        expect(first).toBeDefined();
        // If it works, "你" should align to 1.0.
        // But wait, "你好" will be one word?
        // split(/(\s+)/) on "你好，世界。" -> ["你好，世界。"] (one word).
        // Then normalize("你好，世界。") -> "" if regex strips Chinese.
    });
});
