import { useRef, useState, useMemo, useCallback } from 'react';
import { TranscriptSegment } from '../types/transcript';

/**
 * Hook to track new segments for animation purposes.
 *
 * Identifies segments that haven't been "seen" yet so they can be animated in.
 * Optimized for streaming where segments are appended.
 *
 * @param segments Current list of segments.
 * @return Object containing set of new segment IDs and a callback for when animation ends.
 */
export function useNewSegments(segments: TranscriptSegment[]) {
    // Track which segment IDs have been seen (for animation)
    const knownSegmentIdsRef = useRef<Set<string>>(new Set());
    const prevNewSegmentIdsRef = useRef<Set<string>>(new Set());
    const [animationVersion, setAnimationVersion] = useState(0);

    const handleAnimationEnd = useCallback((id: string) => {
        knownSegmentIdsRef.current.add(id);
        setAnimationVersion(v => v + 1); // Trigger useMemo recomputation
    }, []);

    // Compute new segment IDs synchronously during render
    const newSegmentIds = useMemo(() => {
        const known = knownSegmentIdsRef.current;
        const newIds = new Set<string>();
        let hasNew = false;
        let consecutiveKnowns = 0;

        // Iterate backwards to find new segments efficiently
        for (let i = segments.length - 1; i >= 0; i--) {
            const segment = segments[i];
            if (!known.has(segment.id)) {
                newIds.add(segment.id);
                hasNew = true;
                consecutiveKnowns = 0;
            } else {
                consecutiveKnowns++;
                // Optimization: stop checking after 50 consecutive known segments
                if (consecutiveKnowns >= 50) {
                    break;
                }
            }
        }

        const prev = prevNewSegmentIdsRef.current;

        // Return stable reference if nothing changed
        if (!hasNew && prev.size === 0) {
            return prev;
        }

        if (newIds.size === prev.size) {
            let allSame = true;
            for (const id of newIds) {
                if (!prev.has(id)) {
                    allSame = false;
                    break;
                }
            }
            if (allSame) {
                return prev;
            }
        }

        prevNewSegmentIdsRef.current = newIds;
        return newIds;
    }, [segments, animationVersion]);

    return { newSegmentIds, handleAnimationEnd };
}
