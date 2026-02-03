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

/**
 * Calculates the length of the text excluding punctuation and whitespace.
 * Optimized to avoid unnecessary string allocations.
 */
function getEffectiveLength(text: string): number {
    // Optimization: Check for punctuation first to avoid unnecessary allocation
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
 * Uses token timestamps for precise splitting if available.
 *
 * @param segments - The array of transcript segments to split.
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
            // Optimization: Build a token map for O(1) effective index lookups
            // This replaces the previous O(N) linear scan per character, reducing complexity from O(N^2) to O(N log N)
            tokenMap = buildTokenMap(segment);
        }

        let currentStart = segment.start;
        const totalDuration = segment.end - segment.start;
        const totalLength = text.length;

        // let tokenIndex = 0; // Unused
        let charIndex = 0; // Current character index in the original text
        let effectiveCharIndex = 0; // Current effective character index (ignoring punctuation/spaces)

        let currentText = "";
        let currentSegmentStart = currentStart;

        // Fix: Use the first token's timestamp as the true start time if available
        if (hasTimestamps && tokenMap && tokenMap.timestamps.length > 0) {
            currentSegmentStart = tokenMap.timestamps[0];
        }

        let lastTokenIndex = 0; // Optimization: hint for the next search

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            // Calculate effective length of this part to update the running index
            // We use the same regex as in buildTokenMap to ensure consistency
            // Optimization: Avoid allocation if no punctuation/space
            const partEffectiveLen = getEffectiveLength(part);

            if (SPLIT_REGEX.test(part)) {
                // It's punctuation
                currentText += part;

                let segmentEnd: number;

                if (hasTimestamps && tokenMap) {
                    // Use the last token of the current segment to determine end time
                    // effectiveCharIndex points to the start of the punctuation (and next segment)
                    // so we look at effectiveCharIndex - 1 to find the token for the last character
                    const searchIndex = effectiveCharIndex > 0 ? effectiveCharIndex - 1 : 0;
                    const found = findTimestampFromMap(tokenMap, searchIndex, lastTokenIndex);

                    if (found) {
                        // Use the last token's timestamp plus a small buffer based on length
                        // This avoids using the start time of the *next* segment as the end time
                        const effectiveLen = tokenMap.endIndices[found.index] - tokenMap.startIndices[found.index];
                        const duration = Math.max(0.2, effectiveLen * 0.1);
                        segmentEnd = found.timestamp + duration;
                        lastTokenIndex = found.index;
                    } else {
                        // Fallback
                        segmentEnd = currentStart + (totalLength > 0 ? (currentText.length / totalLength) * totalDuration : 0);
                    }
                } else {
                    segmentEnd = currentStart + (totalLength > 0 ? (currentText.length / totalLength) * totalDuration : 0);
                }

                newSegments.push({
                    id: uuidv4(),
                    text: currentText.trim(),
                    start: currentSegmentStart,
                    end: segmentEnd,
                    isFinal: true
                });

                charIndex += part.length;
                effectiveCharIndex += partEffectiveLen;
                currentStart = segmentEnd;

                // If this isn't the last part, prepare for next segment
                if (i < parts.length - 1) {
                    if (hasTimestamps && tokenMap) {
                        // Find timestamp for charIndex (which is now start of next part)
                        // Effective index is updated.
                        const found = findTimestampFromMap(tokenMap, effectiveCharIndex, lastTokenIndex);
                        if (found) {
                            currentSegmentStart = found.timestamp;
                            currentStart = found.timestamp;
                            lastTokenIndex = found.index;
                        } else {
                            currentSegmentStart = currentStart;
                        }
                    } else {
                        currentSegmentStart = currentStart;
                    }
                }

                currentText = "";
            } else {
                // It's content

                if (currentText === "" && hasTimestamps && tokenMap) {
                    const found = findTimestampFromMap(tokenMap, effectiveCharIndex, lastTokenIndex);
                    if (found) {
                        currentSegmentStart = found.timestamp;
                        lastTokenIndex = found.index;
                    }
                }

                currentText += part;
                charIndex += part.length;
                effectiveCharIndex += partEffectiveLen;
            }
        }

        // Leftover text
        if (currentText.trim()) {
            let segmentEnd = segment.end;

            if (hasTimestamps && tokenMap && tokenMap.timestamps.length > 0) {
                // Try to find the last token
                const searchIndex = effectiveCharIndex > 0 ? effectiveCharIndex - 1 : 0;
                const found = findTimestampFromMap(tokenMap, searchIndex, lastTokenIndex);
                if (found) {
                    const effectiveLen = tokenMap.endIndices[found.index] - tokenMap.startIndices[found.index];
                    const duration = Math.max(0.2, effectiveLen * 0.1);
                    segmentEnd = found.timestamp + duration;
                }
            }

            newSegments.push({
                id: uuidv4(),
                text: currentText.trim(),
                start: currentSegmentStart,
                end: segmentEnd,
                isFinal: true
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
 * @param segment - The transcript segment to build the map from.
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
        // Optimization: Check for punctuation first to avoid unnecessary allocation
        const tokenLen = getEffectiveLength(token);

        if (tokenLen > 0) {
            startIndices.push(currentLen);
            currentLen += tokenLen;
            endIndices.push(currentLen);
            // Fix: Token timestamps are relative to the segment start (due to stream resets).
            // Convert them to absolute time by adding segment.start.
            timestamps.push(segment.timestamps[i] + segment.start);
        }
    }

    return { startIndices, endIndices, timestamps };
}

/**
 * Finds the timestamp corresponding to an effective character index from the map.
 *
 * @param map - The token map.
 * @param effectiveIndex - The effective character index to search for.
 * @param hintIndex - An optional hint index to optimize sequential access.
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
 * Assumes segments are sorted by start time.
 * Accepts an optional hint index for O(1) sequential access.
 *
 * @param segments - The list of transcript segments.
 * @param time - The time to search for.
 * @param hintIndex - Optional hint index to start the search from.
 * @return An object containing the segment (if found) and its index.
 */
export function findSegmentAndIndexForTime(
    segments: TranscriptSegment[],
    time: number,
    hintIndex?: number
): { segment: TranscriptSegment | undefined, index: number } {
    // Optimization: Check hint and surrounding indices first (O(1))
    if (hintIndex !== undefined && hintIndex >= 0 && hintIndex < segments.length) {
        // Check current hint
        const seg = segments[hintIndex];
        if (seg.start <= time && time <= seg.end) {
            return { segment: seg, index: hintIndex };
        }

        // Check next segment (common case for forward playback)
        const nextIdx = hintIndex + 1;
        if (nextIdx < segments.length) {
            const nextSeg = segments[nextIdx];
            if (nextSeg.start <= time && time <= nextSeg.end) {
                return { segment: nextSeg, index: nextIdx };
            }
        }

        // Check previous segment (common case for slight rewind/loops)
        const prevIdx = hintIndex - 1;
        if (prevIdx >= 0) {
            const prevSeg = segments[prevIdx];
            if (prevSeg.start <= time && time <= prevSeg.end) {
                return { segment: prevSeg, index: prevIdx };
            }
        }
    }

    let left = 0;
    let right = segments.length - 1;
    let idx = -1;

    // Binary search for the rightmost segment that starts at or before the time
    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (segments[mid].start <= time) {
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
    }

    return { segment: undefined, index: -1 };
}

/**
 * Efficiently finds the segment containing the given time using binary search.
 * Assumes segments are sorted by start time.
 *
 * @param segments - The list of transcript segments.
 * @param time - The time to search for.
 * @return The segment containing the time, or undefined if not found.
 */
export function findSegmentForTime(segments: TranscriptSegment[], time: number): TranscriptSegment | undefined {
    return findSegmentAndIndexForTime(segments, time).segment;
}
