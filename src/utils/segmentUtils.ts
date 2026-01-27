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

        let currentStart = segment.start;
        const totalDuration = segment.end - segment.start;
        const totalLength = text.length;

        // let tokenIndex = 0; // Unused
        let charIndex = 0; // Current character index in the original text

        let currentText = "";
        let currentSegmentStart = currentStart;

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];

            if (splitRegex.test(part)) {
                // It's punctuation
                currentText += part;

                let segmentEnd: number;

                if (hasTimestamps) {
                    // Find the timestamp of the LAST character of this segment (which is the punctuation usually)
                    // or the character just before it if we want to be safe
                    // We use index - 1 because charIndex points to start of `part` (the punctuation),
                    // so charIndex + part.length - 1 is the index of the last char of the punctuation.

                    // However, punctuation often doesn't have its own timestamp if it's attached to the previous word.
                    // But here we are finding the token containing that char.

                    const lastCharIndex = charIndex + part.length - 1;
                    const lastTokenTimestamp = findTimestampForChar(segment, lastCharIndex);

                    if (lastTokenTimestamp !== undefined) {
                        // We don't have token duration, so we estimate.
                        // 0.2s is a reasonable minimum duration for a final syllable/punctuation.
                        // But we must check against the NEXT segment start to ensure we don't overlap if speech is fast.
                        segmentEnd = lastTokenTimestamp + 0.2;
                        // Note: We'll clamp this later if needed, but for now we leave it independent 
                        // so gaps can exist.
                    } else {
                        // Fallback if we can't find the token
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
                currentStart = segmentEnd;

                // If this isn't the last part, prepare for next segment
                if (i < parts.length - 1) {
                    // Update currentSegmentStart for the next iteration
                    if (hasTimestamps) {
                        // Find the timestamp for the character right after this punctuation
                        // This is the CRITICAL fix: Start time should be the start of the next character
                        // We look for the token that corresponds to the text at charIndex
                        const nextTokenTimestamp = findTimestampForChar(segment, charIndex);
                        if (nextTokenTimestamp !== undefined) {
                            currentSegmentStart = nextTokenTimestamp;
                            currentStart = nextTokenTimestamp;
                        } else {
                            // Fallback if timestamp not found
                            currentSegmentStart = currentStart;
                        }
                    } else {
                        currentSegmentStart = currentStart;
                    }
                }

                currentText = "";
            } else {
                // It's content

                // If this is the START of a new segment (currentText is empty), 
                // and we haven't set a precise start time yet (or we just finished a previous one), 
                // ensure we capture the start time correctly.
                if (currentText === "" && hasTimestamps) {
                    const preciseStart = findTimestampForChar(segment, charIndex);
                    if (preciseStart !== undefined) {
                        currentSegmentStart = preciseStart;
                    }
                }

                currentText += part;
                charIndex += part.length;
            }
        }

        // Leftover text
        if (currentText.trim()) {
            const segmentEnd = segment.end;

            // If we have a pending start time from the loop, use it
            // Otherwise currentSegmentStart should be correct from previous iteration

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

/**
 * Helper to approximate the timestamp for a character index using token data.
 * Returns the timestamp of the token that best contains the character at `charIndex`.
 */
function findTimestampForChar(segment: TranscriptSegment, charIndex: number): number | undefined {
    if (!segment.tokens || !segment.timestamps) return undefined;

    // Calculate effective index (ignoring whitespace) to handle drift between text and tokens
    // (e.g. text has spaces that are not present or accounted for in token lengths)
    const textUpToChar = segment.text.slice(0, charIndex);
    const effectiveIndex = textUpToChar.replace(/\s/g, '').length;

    let currentLen = 0;
    for (let i = 0; i < segment.tokens.length; i++) {
        const token = segment.tokens[i];

        // Check if the token covers the effective index
        // We assume tokens correspond to the non-whitespace content of the text.
        if (effectiveIndex >= currentLen && effectiveIndex < currentLen + token.length) {
            return segment.timestamps[i];
        }

        currentLen += token.length;
    }

    // If we're past the end, return the last timestamp or undefined
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
