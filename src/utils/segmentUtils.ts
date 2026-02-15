import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';

/**
 * Split segments by punctuation, using token timestamps if available for better accuracy.
 */
// Regular expressions for text processing
// Hoisted to module scope to avoid reallocation in loops
const SPLIT_REGEX = /([.?!。？！]+)/;
const PUNCTUATION_REGEX = /[\s\p{P}]/u;
const PUNCTUATION_REPLACE_REGEX = /[\s\p{P}]+/gu;

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
 * Splits transcript segments based on punctuation marks.
 *
 * Uses token timestamps for precise splitting if available.
 *
 * @param segments The array of transcript segments to split.
 * @return A new array of split transcript segments.
 */
export function splitByPunctuation(segments: TranscriptSegment[]): TranscriptSegment[] {
    const newSegments: TranscriptSegment[] = [];

    segments.forEach(segment => {
        const text = segment.text;
        const parts = text.split(SPLIT_REGEX);

        // If we have token timestamps, we can be very precise
        // Otherwise we fall back to character length interpolation
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

            if (SPLIT_REGEX.test(part)) {
                // It's punctuation
                currentText += part;

                // Advance effective index first to capture the punctuation's place in the map 
                // (though for punctuation len is 0, so it stays same, but conceptually we are "after" it)
                charIndex += part.length;
                effectiveCharIndex += partEffectiveLen;

                let segmentEnd: number;
                let currentTokens: string[] | undefined;
                let currentTimestamps: number[] | undefined;

                if (hasTimestamps && tokenMap) {
                    // Find the token at the current boundary
                    const found = findTimestampFromMap(tokenMap, effectiveCharIndex, lastTokenIndex);

                    // End index for slicing: if found, slice up to found.index (exclusive? no, see prior logic).
                    // findTimestampFromMap returns the token matching effectiveCharIndex.
                    // If effectiveCharIndex is 10, and "How" starts at 10, it returns "How".
                    // We want to slice UP TO "How" (exclusive).

                    let sliceEnd = tokenMap.timestamps.length; // Default to all remaining

                    if (found) {
                        sliceEnd = found.index;

                        // Calculate time
                        const effectiveLen = tokenMap.endIndices[found.index] - tokenMap.startIndices[found.index];
                        const duration = Math.max(0.2, effectiveLen * 0.1);
                        segmentEnd = found.timestamp + duration; // Approximate end time based on next start? No that's weird.

                        // Actually better: use found timestamp as start of NEXT.
                        // End of CURRENT is found.timestamp.
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

                    // Fix start time if we have tokens
                    if (currentTimestamps && currentTimestamps.length > 0) {
                        currentSegmentStart = currentTimestamps[0];
                    }

                } else {
                    segmentEnd = currentStart + (totalLength > 0 ? (currentText.length / totalLength) * totalDuration : 0);
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

                currentStart = segmentEnd;
                currentSegmentStart = segmentEnd; // Default for next

                // If found valid next token start, use it
                if (hasTimestamps && tokenMap && lastTokenIndex < tokenMap.timestamps.length) {
                    // Verify if lastTokenIndex (which is `found.index` i.e. "How") is indeed the start
                    // sliceEnd was found.index. So next starts at found.index.
                    // The token at found.index is the start of next segment.
                    // Its timestamp is in tokenMap.timestamps[found.index].
                    if (lastTokenIndex === nextTokenSliceStart) { // Use slice tracker as truth
                        if (nextTokenSliceStart < tokenMap.timestamps.length) {
                            currentSegmentStart = tokenMap.timestamps[nextTokenSliceStart];
                            currentStart = currentSegmentStart;
                        }
                    }
                }

                currentText = "";
            } else {
                // It's content, tokenize logic handled at split
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
                // Slice everything remaining
                currentTokens = segment.tokens!.slice(nextTokenSliceStart);
                currentTimestamps = segment.timestamps!.slice(nextTokenSliceStart);

                if (currentTimestamps.length > 0) {
                    currentSegmentStart = currentTimestamps[0];

                    // Try to refine end time based on last token?
                    // Not critical.
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
    });

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
