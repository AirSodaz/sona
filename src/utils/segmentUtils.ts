import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';

// Constants
const MAX_SEGMENT_LENGTH_CJK = 36;
const MAX_SEGMENT_LENGTH_WESTERN = 84;

// Common abbreviations that shouldn't trigger a sentence split
const ABBREVIATIONS = new Set([
    'mr', 'mrs', 'ms', 'dr', 'prof', 'sr', 'jr', 'st', 'vs', 'etc',
    'no', 'op', 'vol', 'fig', 'inc', 'ltd', 'co', 'dept'
]);

// Regular expressions for text processing
// Hoisted to module scope to avoid reallocation in loops
const SPLIT_REGEX = /([.?!。？！]+)/;
const COMMA_SPLIT_REGEX = /([,，;；:：]+)/;
const PUNCTUATION_REGEX = /[\s\p{P}]/u;
const PUNCTUATION_REPLACE_REGEX = /[\s\p{P}]+/gu;

// Regex for detection
const CJK_REGEX = /[\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}\p{sc=Hangul}]/u;

// Regex for alignTokensToText
const NORMALIZE_REGEX = /[^\p{L}\p{N}]/gu;
const RAW_WORDS_REGEX = /(\s+|[\p{sc=Han}]|[^\s\p{sc=Han}]+)/gu;
const PUNCTUATION_ONLY_REGEX = /^[^\p{L}\p{N}]+$/u;
const WHITESPACE_ONLY_REGEX = /^\s+$/;

/**
 * Calculates the length of the text excluding punctuation and whitespace.
 */
function getEffectiveLength(text: string): number {
    // Check for punctuation first to avoid unnecessary allocation
    if (!PUNCTUATION_REGEX.test(text)) {
        return text.length;
    }

    let reduction = 0;
    // Reset lastIndex for the global regex before reuse
    PUNCTUATION_REPLACE_REGEX.lastIndex = 0;
    let match;
    while ((match = PUNCTUATION_REPLACE_REGEX.exec(text)) !== null) {
        reduction += match[0].length;
    }
    return text.length - reduction;
}

/**
 * Checks if the given text ends with a known abbreviation.
 */
function endsWithAbbreviation(text: string): boolean {
    const trimmed = text.trim();
    if (!trimmed) return false;
    // Get last word (simplistic split by space is usually sufficient for this check)
    const words = trimmed.split(/\s+/);
    const lastWord = words[words.length - 1].toLowerCase();
    return ABBREVIATIONS.has(lastWord);
}

/**
 * Determines if text contains CJK characters to decide on segment length limit.
 */
function isCJK(text: string): boolean {
    return CJK_REGEX.test(text);
}

/**
 * Splits transcript segments based on punctuation marks.
 * Uses a two-pass approach:
 * 1. Split by sentence-ending punctuation (handling abbreviations).
 * 2. Split long segments by weaker punctuation (commas) based on language constraints.
 *    - CJK: 36 characters
 *    - Western: 84 characters
 *
 * @param segments The array of transcript segments to split.
 * @return A new array of split transcript segments.
 */
export function splitByPunctuation(segments: TranscriptSegment[]): TranscriptSegment[] {
    // Pass 1: Sentence Splitting (Strong Punctuation)
    let intermediateSegments: TranscriptSegment[] = [];

    for (const segment of segments) {
        const parts = splitSegmentByRegex(segment, SPLIT_REGEX, { checkAbbreviations: true });
        intermediateSegments.push(...parts);
    }

    // Pass 2: Length Constraints (Weak Punctuation)
    const finalSegments: TranscriptSegment[] = [];

    for (const segment of intermediateSegments) {
        const limit = isCJK(segment.text) ? MAX_SEGMENT_LENGTH_CJK : MAX_SEGMENT_LENGTH_WESTERN;

        // If segment is too long, try to split by commas
        if (segment.text.length > limit) {
            const subSegments = splitSegmentByRegex(segment, COMMA_SPLIT_REGEX, { checkAbbreviations: false });
            // If splitting happened (length > 1), we use the sub-segments.
            // Even if length is 1 (no comma found), we push that 1 segment.
            finalSegments.push(...subSegments);
        } else {
            finalSegments.push(segment);
        }
    }

    return finalSegments;
}

interface SplitOptions {
    checkAbbreviations: boolean;
}

/**
 * Helper to split a single segment by a given regex pattern.
 * Handles token/timestamp interpolation and abbreviation checks.
 */
function splitSegmentByRegex(segment: TranscriptSegment, regex: RegExp, options: SplitOptions): TranscriptSegment[] {
    const newSegments: TranscriptSegment[] = [];
    const text = segment.text;
    const parts = text.split(regex);

    // Optimization: If no split happened, return original (cloned with new ID if needed, or just original?)
    // To be safe and consistent, we'll process it. But if parts.length === 1, it's just one segment.
    if (parts.length <= 1) {
        return [{...segment, id: uuidv4()}]; // Return copy with new ID for consistency? Or keep original ID?
        // Actually, best to just return the segment itself to avoid unnecessary ID churn if no change?
        // But `splitByPunctuation` usually generates new IDs.
        // Let's assume consistent ID generation is safer.
    }

    const hasTimestamps = segment.tokens && segment.timestamps &&
        segment.tokens.length === segment.timestamps.length;

    let tokenMap: TokenMap | null = null;
    if (hasTimestamps) {
        tokenMap = buildTokenMap(segment);
    }

    let currentStart = segment.start;
    const totalDuration = segment.end - segment.start;
    const totalLength = text.length;

    let charIndex = 0; // Current character index in the original text
    let effectiveCharIndex = 0; // Current effective character index (ignoring punctuation/spaces)

    let currentText = "";
    let currentSegmentStart = currentStart;

    // Fix: Use the first token's timestamp as the true start time if available
    if (hasTimestamps && tokenMap && tokenMap.timestamps.length > 0) {
        currentSegmentStart = tokenMap.timestamps[0];
    }

    let lastTokenIndex = 0; // Hint for the next search
    let nextTokenSliceStart = 0; // Track token slicing

    for (let i = 0; i < parts.length; i++) {
        const part = parts[i];
        const partEffectiveLen = getEffectiveLength(part);

        if (regex.test(part)) {
            // It's a delimiter (punctuation)

            // Check for abbreviation if enabled
            // Only relevant if the delimiter starts with '.' (e.g. SPLIT_REGEX)
            // But we can just check `options.checkAbbreviations`
            let shouldMerge = false;
            if (options.checkAbbreviations && part.includes('.')) {
                 if (endsWithAbbreviation(currentText)) {
                     shouldMerge = true;
                 }
            }

            if (shouldMerge) {
                // Treat delimiter as content
                currentText += part;
                charIndex += part.length;
                effectiveCharIndex += partEffectiveLen;
                // Continue to next part
                continue;
            }

            // Normal split
            currentText += part;
            charIndex += part.length;
            effectiveCharIndex += partEffectiveLen;

            let segmentEnd: number;
            let currentTokens: string[] | undefined;
            let currentTimestamps: number[] | undefined;

            if (hasTimestamps && tokenMap) {
                // Find the token at the current boundary
                const found = findTimestampFromMap(tokenMap, effectiveCharIndex, lastTokenIndex);

                let sliceEnd = tokenMap.timestamps.length;

                if (found) {
                    sliceEnd = found.index;
                    // End of CURRENT segment is the start of the token FOLLOWING this text block.
                    // Wait, `found` is the token that *starts* at or after effectiveCharIndex.
                    // So yes, found.timestamp is the split point.
                    segmentEnd = found.timestamp;
                    lastTokenIndex = found.index;
                } else {
                    // End of stream or drift
                    segmentEnd = currentStart + (totalLength > 0 ? (currentText.length / totalLength) * totalDuration : 0);
                }

                // Extract tokens
                if (sliceEnd > nextTokenSliceStart) {
                    currentTokens = segment.tokens!.slice(nextTokenSliceStart, sliceEnd);
                    currentTimestamps = segment.timestamps!.slice(nextTokenSliceStart, sliceEnd);
                    nextTokenSliceStart = sliceEnd;
                }

                if (currentTimestamps && currentTimestamps.length > 0) {
                    currentSegmentStart = currentTimestamps[0];
                }

            } else {
                segmentEnd = currentStart + (totalLength > 0 ? (currentText.length / totalLength) * totalDuration : 0);
            }

            // Push the segment
            newSegments.push({
                id: uuidv4(),
                text: currentText.trim(),
                start: currentSegmentStart,
                end: segmentEnd,
                isFinal: true,
                tokens: currentTokens,
                timestamps: currentTimestamps
            });

            // Prepare for next segment
            currentStart = segmentEnd;
            currentSegmentStart = segmentEnd; // Default

            // If found valid next token start, use it
            if (hasTimestamps && tokenMap && lastTokenIndex < tokenMap.timestamps.length) {
                if (lastTokenIndex === nextTokenSliceStart) {
                    if (nextTokenSliceStart < tokenMap.timestamps.length) {
                        currentSegmentStart = tokenMap.timestamps[nextTokenSliceStart];
                        currentStart = currentSegmentStart;
                    }
                }
            }

            currentText = "";

        } else {
            // Content
            currentText += part;
            charIndex += part.length;
            effectiveCharIndex += partEffectiveLen;
        }
    }

    // Leftover text
    if (currentText.trim()) {
        let segmentEnd = segment.end;
        let currentTokens: string[] | undefined;
        let currentTimestamps: number[] | undefined;

        if (hasTimestamps && tokenMap) {
            currentTokens = segment.tokens!.slice(nextTokenSliceStart);
            currentTimestamps = segment.timestamps!.slice(nextTokenSliceStart);

            if (currentTimestamps.length > 0) {
                currentSegmentStart = currentTimestamps[0];
            }
        }

        newSegments.push({
            id: uuidv4(),
            text: currentText.trim(),
            start: currentSegmentStart,
            end: segmentEnd,
            isFinal: true,
            tokens: currentTokens,
            timestamps: currentTimestamps
        });
    }

    return newSegments;
}

/** Pre-calculated map for faster lookups. */
interface TokenMap {
    /** Start effective index of each token. */
    startIndices: number[];
    /** End effective index of each token. */
    endIndices: number[];
    /** Timestamp of each token. */
    timestamps: number[];
}

/**
 * Builds a map of tokens to their timestamps and effective indices.
 *
 * @param segment The transcript segment to build the map from.
 * @return The token map, or null if tokens or timestamps are missing.
 */
function buildTokenMap(segment: TranscriptSegment): TokenMap | null {
    if (!segment.tokens || !segment.timestamps) return null;

    const startIndices: number[] = [];
    const endIndices: number[] = [];
    const timestamps: number[] = [];

    let currentLen = 0;

    for (let i = 0; i < segment.tokens.length; i++) {
        const token = segment.tokens[i];
        // Strip punctuation and whitespace
        const tokenLen = getEffectiveLength(token);

        // Include all tokens, even whitespace/punctuation, to maintain alignment
        startIndices.push(currentLen);
        currentLen += tokenLen;
        endIndices.push(currentLen);
        // Token timestamps are already absolute
        timestamps.push(segment.timestamps[i]);
    }

    return { startIndices, endIndices, timestamps };
}

/**
 * Finds the timestamp corresponding to an effective character index from the map.
 *
 * @param map The token map.
 * @param effectiveIndex The effective character index to search for.
 * @param hintIndex An optional hint index to optimize sequential access.
 * @return An object containing the timestamp and the index in the map, or undefined if not found.
 */
function findTimestampFromMap(map: TokenMap, effectiveIndex: number, hintIndex: number = 0): { timestamp: number, index: number } | undefined {
    // Check hint first for O(1) access
    if (hintIndex < map.startIndices.length) {
        if (map.startIndices[hintIndex] <= effectiveIndex && effectiveIndex < map.endIndices[hintIndex]) {
            return { timestamp: map.timestamps[hintIndex], index: hintIndex };
        }
        // Check next token (common case for sequential access)
        const next = hintIndex + 1;
        if (next < map.startIndices.length) {
            if (map.startIndices[next] <= effectiveIndex && effectiveIndex < map.endIndices[next]) {
                return { timestamp: map.timestamps[next], index: next };
            }
        }
    }

    // Binary search to find the token that covers effectiveIndex
    // We are looking for idx where startIndices[idx] <= effectiveIndex < endIndices[idx]

    let left = hintIndex;
    let right = map.startIndices.length - 1;
    let idx = -1;

    // If hint was past the target (shouldn't happen with sequential access), reset left
    if (left > right || (left < map.startIndices.length && map.startIndices[left] > effectiveIndex)) {
        left = 0;
    }

    // Find rightmost start <= effectiveIndex
    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (map.startIndices[mid] <= effectiveIndex) {
            idx = mid;
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    if (idx !== -1) {
        // Check if effectiveIndex is strictly within this token (or at start)
        if (effectiveIndex < map.endIndices[idx]) {
            return { timestamp: map.timestamps[idx], index: idx };
        }
    }
    return undefined;
}

/**
 * Efficiently finds the segment containing the given time using binary search.
 *
 * Assumes segments are sorted by start time. Accepts an optional hint index for O(1) sequential access.
 *
 * @param segments The list of transcript segments.
 * @param time The time to search for.
 * @param hintIndex Optional hint index to start the search from.
 * @return An object containing the segment (if found) and its index.
 */
export function findSegmentAndIndexForTime(
    segments: TranscriptSegment[],
    time: number,
    hintIndex?: number
): { segment: TranscriptSegment | undefined, index: number } {
    // Tolerance for floating point precision issues and micro-gaps (e.g. 50ms)
    const EPSILON = 0.05;
    const searchTime = time + EPSILON;

    // Optimization: Check hint and surrounding indices first (O(1))
    if (hintIndex !== undefined && hintIndex >= -1 && hintIndex < segments.length) {

        // Handle pre-roll (hint -1)
        if (hintIndex === -1) {
            if (segments.length > 0 && time < segments[0].start) {
                return { segment: undefined, index: -1 };
            }
            if (segments.length > 0 && time < segments[0].end && time >= segments[0].start) {
                return { segment: segments[0], index: 0 };
            }
        } else {
            // Check current hint
            const seg = segments[hintIndex];
            if (seg.start <= searchTime && time < seg.end) {
                return { segment: seg, index: hintIndex };
            }

            // Check next segment (common case for forward playback)
            const nextIdx = hintIndex + 1;
            if (nextIdx < segments.length) {
                const nextSeg = segments[nextIdx];
                if (nextSeg.start <= searchTime && time < nextSeg.end) {
                    return { segment: nextSeg, index: nextIdx };
                }
            }

            // Check gap after hint (time >= seg.end AND (next doesn't exist OR time < next.start))
            if (time >= seg.end) {
                if (nextIdx >= segments.length || searchTime < segments[nextIdx].start) {
                    return { segment: undefined, index: hintIndex };
                }
            }

            // Check previous segment (common case for slight rewind/loops)
            const prevIdx = hintIndex - 1;
            if (prevIdx >= 0) {
                const prevSeg = segments[prevIdx];
                if (prevSeg.start <= searchTime && time < prevSeg.end) {
                    return { segment: prevSeg, index: prevIdx };
                }

                // Check gap after prev (time >= prevSeg.end AND time < seg.start)
                if (time >= prevSeg.end && searchTime < seg.start) {
                    return { segment: undefined, index: prevIdx };
                }
            }
        }
    }

    let left = 0;
    let right = segments.length - 1;
    let idx = -1;

    // Binary search for the rightmost segment that starts at or before the time
    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (segments[mid].start <= searchTime) {
            idx = mid;
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    if (idx !== -1) {
        const seg = segments[idx];
        if (time <= seg.end) {
            return { segment: seg, index: idx };
        }
        // If time > seg.end, we are in the gap after 'idx'.
        // Return 'idx' as the "closest preceding segment index".
        return { segment: undefined, index: idx };
    }

    return { segment: undefined, index: -1 };
}

/**
 * Efficiently finds the segment containing the given time using binary search.
 *
 * Assumes segments are sorted by start time.
 *
 * @param segments The list of transcript segments.
 * @param time The time to search for.
 * @return The segment containing the time, or undefined if not found.
 */
export function findSegmentForTime(segments: TranscriptSegment[], time: number): TranscriptSegment | undefined {
    return findSegmentAndIndexForTime(segments, time).segment;
}

/**
 * Aligns formatted text with raw tokens to assign timestamps to display words.
 * 
 * @param text The formatted text (with punctuation/ITN).
 * @param rawTokens The raw tokens from the recognizer.
 * @param rawTimestamps The timestamps corresponding to rawTokens.
 * @return Array of objects with text chunk and its start timestamp.
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

    // Pre-normalize tokens to avoid repeated regex and toLowerCase calls
    const normalizedTokens = rawTokens.map(t => t.toLowerCase().replace(NORMALIZE_REGEX, ''));

    // Tokenize text:
    // 1. Whitespace (kept to preserve spacing)
    // 2. Chinese characters (Han script) treated as individual words
    // 3. Everything else (English, numbers, punctuation) grouped until whitespace/Han/End
    const rawWords = text.match(RAW_WORDS_REGEX) || [];

    // Merge standalone punctuation into the previous word
    const words: string[] = [];
    const normalizedWords: string[] = [];

    for (const w of rawWords) {
        // Check if w is purely punctuation (and previous word exists and is not whitespace)
        if (words.length > 0 && PUNCTUATION_ONLY_REGEX.test(w) && !WHITESPACE_ONLY_REGEX.test(w) && !WHITESPACE_ONLY_REGEX.test(words[words.length - 1])) {
            // Append to previous word
            words[words.length - 1] += w;
            // Update normalized cache for the merged word
            normalizedWords[normalizedWords.length - 1] = words[words.length - 1].toLowerCase().replace(NORMALIZE_REGEX, '');
        } else {
            words.push(w);
            normalizedWords.push(w.toLowerCase().replace(NORMALIZE_REGEX, ''));
        }
    }

    let currentRawIndex = 0;
    const maxTokens = rawTokens.length;

    for (let i = 0; i < words.length; i++) {
        const word = words[i];

        // Skip whitespace words but preserve them in output
        if (!word.trim()) {
            const ts = rawTimestamps.length > 0 ? rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)] : 0;
            result.push({ text: word, timestamp: ts });
            continue;
        }

        const normWord = normalizedWords[i];
        if (!normWord) {
            const ts = rawTimestamps.length > 0 ? rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)] : 0;
            result.push({ text: word, timestamp: ts });
            continue;
        }

        const startTimestamp = rawTimestamps.length > 0
            ? rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)]
            : 0;
        result.push({ text: word, timestamp: startTimestamp });

        // Try to match `normWord` against next N tokens.
        let accumulatedTokenStr = "";
        let tokensConsumed = 0;
        let foundMatch = false;

        for (let j = 0; j < 5 && (currentRawIndex + j) < maxTokens; j++) {
            const t = normalizedTokens[currentRawIndex + j];
            accumulatedTokenStr += t;
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
                nextNorm = normalizedWords[nextIdx];
                if (nextNorm) break;
            }

            if (nextNorm) {
                // distinct next word
                // Scan ahead in tokens to find `nextNorm`.
                for (let k = 1; k < 10 && (currentRawIndex + k) < maxTokens; k++) {
                    const t = normalizedTokens[currentRawIndex + k];
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
                // Consume all remaining tokens if reasonable
                let hasRemainingContent = false;
                for (let r = i + 1; r < words.length; r++) {
                    if (normalizedWords[r]) {
                        hasRemainingContent = true;
                        break;
                    }
                }

                if (i === words.length - 1 || (i > words.length - 3 && !hasRemainingContent)) {
                    currentRawIndex = maxTokens;
                    foundMatch = true;
                }
            }

            if (!foundMatch) {
                // Fallback: Just consume 1 token.
                currentRawIndex++;
            }
        }

        if (currentRawIndex >= maxTokens) currentRawIndex = maxTokens;
    }

    return result;
}
