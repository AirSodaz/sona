import { useMemo, useRef, useLayoutEffect, useEffect, MutableRefObject } from 'react';
import { StoreApi } from 'zustand';
import { useTranscriptStore } from '../stores/transcriptStore';
import { TranscriptSegment } from '../types/transcript';
import { TranscriptUIState } from '../components/transcript/TranscriptUIContext';

/**
 * Hook to track new segments for animation and sync UI state.
 *
 * This hook manages the identification of "new" segments (for enter animations)
 * and synchronizes the local UI store with global transcript store changes
 * (active, editing, and aligning states).
 *
 * @param segments The current list of segments.
 * @param uiStore The local UI store for the transcript editor.
 * @param animationVersion A version number that increments when an animation ends, triggering re-computation.
 * @return An object containing the new segment IDs and a ref to the known segment IDs.
 */
export function useNewSegments(
    segments: TranscriptSegment[],
    uiStore: StoreApi<TranscriptUIState>,
    animationVersion: number
): {
    newSegmentIds: Set<string>;
    knownSegmentIdsRef: MutableRefObject<Set<string>>;
} {
    // Track which segment IDs have been seen (for animation)
    const knownSegmentIdsRef = useRef<Set<string>>(new Set());
    const prevNewSegmentIdsRef = useRef<Set<string>>(new Set());

    // Compute new segment IDs synchronously during render
    const newSegmentIds = useMemo(() => {
        const known = knownSegmentIdsRef.current;
        const newIds = new Set<string>();
        let hasNew = false;
        let consecutiveKnowns = 0;

        for (let i = segments.length - 1; i >= 0; i--) {
            const segment = segments[i];
            if (!known.has(segment.id)) {
                newIds.add(segment.id);
                hasNew = true;
                consecutiveKnowns = 0;
            } else {
                consecutiveKnowns++;
                // Optimization: Stop checking after 50 known segments
                // This prevents scanning the entire list for every render on large transcripts
                if (consecutiveKnowns >= 50) {
                    break;
                }
            }
        }

        const prev = prevNewSegmentIdsRef.current;

        // If no new segments and no previous new segments, return stable reference
        if (!hasNew && prev.size === 0) {
            return prev;
        }

        // Stability check to return same Set reference if content is identical
        // This prevents unnecessary re-renders in consumers
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

    // Sync newSegmentIds and totalSegments to local store
    // useLayoutEffect ensures this happens before paint
    useLayoutEffect(() => {
        uiStore.setState({
            newSegmentIds,
            totalSegments: segments.length
        });
    }, [newSegmentIds, segments.length, uiStore]);

    // Sync activeSegmentId, editingSegmentId, and aligningSegmentIds from global store to local store
    // This avoids the main TranscriptEditor component re-rendering when these high-frequency or specific states change
    useEffect(() => {
        return useTranscriptStore.subscribe((state, prevState) => {
            const updates: Partial<TranscriptUIState> = {};
            let hasUpdates = false;

            if (state.activeSegmentId !== prevState.activeSegmentId) {
                updates.activeSegmentId = state.activeSegmentId;
                hasUpdates = true;
            }
            if (state.editingSegmentId !== prevState.editingSegmentId) {
                updates.editingSegmentId = state.editingSegmentId;
                hasUpdates = true;
            }
            if (state.aligningSegmentIds !== prevState.aligningSegmentIds) {
                updates.aligningSegmentIds = state.aligningSegmentIds;
                hasUpdates = true;
            }

            if (hasUpdates) {
                uiStore.setState(updates);
            }
        });
    }, [uiStore]);

    return {
        newSegmentIds,
        knownSegmentIdsRef
    };
}
