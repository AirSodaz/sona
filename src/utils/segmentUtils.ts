import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';

/**
 * Split segments by punctuation, using token timestamps if available for better accuracy.
 */
export function splitByPunctuation(segments: TranscriptSegment[]): TranscriptSegment[] {
    const newSegments: TranscriptSegment[] = [];
    const splitRegex = /([.?!。？！]+)/;

    segments.forEach(segment => {
        const text = segment.text;
        const parts = text.split(splitRegex);

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

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            // Calculate effective length of this part to update the running index
            // We use the same regex as in buildTokenMap to ensure consistency
            const partEffectiveLen = part.replace(/[\s\p{P}]+/gu, '').length;

            if (splitRegex.test(part)) {
                // It's punctuation
                currentText += part;

                let segmentEnd: number;

                if (hasTimestamps && tokenMap) {
                    const lastTokenTimestamp = findTimestampFromMap(tokenMap, effectiveCharIndex);

                    if (lastTokenTimestamp !== undefined) {
                        segmentEnd = lastTokenTimestamp + 0.2;
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
                        const nextTokenTimestamp = findTimestampFromMap(tokenMap, effectiveCharIndex);
                        if (nextTokenTimestamp !== undefined) {
                            currentSegmentStart = nextTokenTimestamp;
                            currentStart = nextTokenTimestamp;
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
                    const preciseStart = findTimestampFromMap(tokenMap, effectiveCharIndex);
                    if (preciseStart !== undefined) {
                        currentSegmentStart = preciseStart;
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
        const tokenLen = token.replace(/[\s\p{P}]+/gu, '').length;

        if (tokenLen > 0) {
            startIndices.push(currentLen);
            currentLen += tokenLen;
            endIndices.push(currentLen);
            timestamps.push(segment.timestamps[i]);
        }
    }

    return { startIndices, endIndices, timestamps };
}

function findTimestampFromMap(map: TokenMap, effectiveIndex: number): number | undefined {
    // Binary search to find the token that covers effectiveIndex
    // We are looking for idx where startIndices[idx] <= effectiveIndex < endIndices[idx]

    let left = 0;
    let right = map.startIndices.length - 1;
    let idx = -1;

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
            return map.timestamps[idx];
        }
    }
    return undefined;
}

/**
 * Efficiently finds the segment containing the given time using binary search.
 * Assumes segments are sorted by start time.
 * Returns undefined if no segment covers the time.
 */
export function findSegmentForTime(segments: TranscriptSegment[], time: number): TranscriptSegment | undefined {
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
            return seg;
        }
    }

    return undefined;
}
