import { useMemo, useRef, useLayoutEffect, useEffect, useCallback } from 'react';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore } from '../stores/transcriptStore';
import { TranscriptUIState } from '../components/transcript/TranscriptUIContext';
import { TranscriptSegment } from '../types/transcript';

/**
 * Hook to manage UI state for the transcript editor, specifically efficient re-rendering
 * of list items and animation of new segments.
 */
export function useTranscriptUIState(segments: TranscriptSegment[]) {
    // Track which segment IDs have been seen (for animation)
    const knownSegmentIdsRef = useRef<Set<string>>(new Set());
    const prevNewSegmentIdsRef = useRef<Set<string>>(new Set());

    // Create a local store for UI state (newSegmentIds) to prevent Context updates
    // from re-rendering the entire list.
    const uiStore = useMemo(() => createStore<TranscriptUIState>(() => ({
        newSegmentIds: new Set(),
        activeSegmentId: useTranscriptStore.getState().activeSegmentId,
        editingSegmentId: useTranscriptStore.getState().editingSegmentId,
        totalSegments: useTranscriptStore.getState().segments.length,
        aligningSegmentIds: useTranscriptStore.getState().aligningSegmentIds,
    })), []);

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

                // Optimization: If there are too many new segments (e.g. bulk load),
                // prevent O(N) calculations on subsequent renders by marking all as known immediately.
                if (newIds.size > 50) {
                    knownSegmentIdsRef.current = new Set(segments.map(s => s.id));
                    return new Set<string>();
                }
            } else {
                consecutiveKnowns++;
                if (consecutiveKnowns >= 50) {
                    break;
                }
            }
        }

        const prev = prevNewSegmentIdsRef.current;

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
    }, [segments]);

    // Sync newSegmentIds and totalSegments to local store
    useLayoutEffect(() => {
        uiStore.setState({
            newSegmentIds,
            totalSegments: segments.length
        });
    }, [newSegmentIds, segments.length, uiStore]);

    // Sync activeSegmentId, editingSegmentId, and aligningSegmentIds from global store to local store
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

    const handleAnimationEnd = useCallback((id: string) => {
        knownSegmentIdsRef.current.add(id);

        // Update store directly to remove from newSegmentIds without triggering full re-render
        uiStore.setState(state => {
            if (!state.newSegmentIds.has(id)) return state;

            const next = new Set(state.newSegmentIds);
            next.delete(id);
            return { newSegmentIds: next };
        });
    }, [uiStore]);

    return { uiStore, handleAnimationEnd };
}
