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
        let lastTokenIndex = 0; // Optimization: hint for the next search

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            // Calculate effective length of this part to update the running index
            // We use the same regex as in buildTokenMap to ensure consistency
            // Optimization: Avoid allocation if no punctuation/space
            const partEffectiveLen = PUNCTUATION_REGEX.test(part)
                ? part.replace(PUNCTUATION_REPLACE_REGEX, '').length
                : part.length;

            if (SPLIT_REGEX.test(part)) {
                // It's punctuation
                currentText += part;

                let segmentEnd: number;

                if (hasTimestamps && tokenMap) {
                    const found = findTimestampFromMap(tokenMap, effectiveCharIndex, lastTokenIndex);

                    if (found) {
                        segmentEnd = found.timestamp + 0.2;
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
            const segmentEnd = segment.end;
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

// Pre-calculated map for faster lookups
interface TokenMap {
    startIndices: number[]; // Start effective index of each token
    endIndices: number[];   // End effective index of each token
    timestamps: number[];   // Timestamp of each token
}

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
        const tokenLen = PUNCTUATION_REGEX.test(token)
            ? token.replace(PUNCTUATION_REPLACE_REGEX, '').length
            : token.length;

        if (tokenLen > 0) {
            startIndices.push(currentLen);
            currentLen += tokenLen;
            endIndices.push(currentLen);
            timestamps.push(segment.timestamps[i]);
        }
    }

    return { startIndices, endIndices, timestamps };
}

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
 * Returns the segment and its index. If not found, segment is undefined and index is -1.
 */
export function findSegmentForTime(segments: TranscriptSegment[], time: number, hintIndex: number = -1): { segment: TranscriptSegment | undefined, index: number } {
    // Optimization: Check hint index first (O(1) access)
    // This is useful for sequential playback where we likely need the same or next segment
    if (hintIndex >= 0 && hintIndex < segments.length) {
        // Check current hint
        const seg = segments[hintIndex];
        if (time >= seg.start && time <= seg.end) {
            return { segment: seg, index: hintIndex };
        }

        // Check next segment (common case for forward playback)
        if (hintIndex + 1 < segments.length) {
            const nextSeg = segments[hintIndex + 1];
            if (time >= nextSeg.start && time <= nextSeg.end) {
                return { segment: nextSeg, index: hintIndex + 1 };
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
