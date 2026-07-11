import { useMemo, useRef, useLayoutEffect, useEffect, useCallback } from 'react';
import { createStore } from 'zustand/vanilla';
import { TranscriptUIState } from '../components/transcript/TranscriptUIContext';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
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
        activeSegmentId: useTranscriptPlaybackStore.getState().activeSegmentId,
        editingSegmentId: useTranscriptSessionStore.getState().editingSegmentId,
        totalSegments: useTranscriptSessionStore.getState().segments.length,
        aligningSegmentIds: useTranscriptSessionStore.getState().aligningSegmentIds,
    })), []);

    useLayoutEffect(() => {
        const known = knownSegmentIdsRef.current;
        const prev = prevNewSegmentIdsRef.current;
        const newIds = new Set<string>();
        let hasNew = false;
        let consecutiveKnowns = 0;
        let nextNewSegmentIds: Set<string>;

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
                    prevNewSegmentIdsRef.current = new Set<string>();
                    uiStore.setState({
                        newSegmentIds: prevNewSegmentIdsRef.current,
                        totalSegments: segments.length
                    });
                    return;
                }
            } else {
                consecutiveKnowns++;
                if (consecutiveKnowns >= 50) {
                    break;
                }
            }
        }

        if (!hasNew && prev.size === 0) {
            nextNewSegmentIds = prev;
        } else if (newIds.size === prev.size) {
            let allSame = true;
            for (const id of newIds) {
                if (!prev.has(id)) {
                    allSame = false;
                    break;
                }
            }
            if (allSame) {
                nextNewSegmentIds = prev;
            } else {
                nextNewSegmentIds = newIds;
            }
        } else {
            nextNewSegmentIds = newIds;
        }

        prevNewSegmentIdsRef.current = nextNewSegmentIds;
        uiStore.setState({
            newSegmentIds: nextNewSegmentIds,
            totalSegments: segments.length
        });
    }, [segments, uiStore]);

    // Sync activeSegmentId, editingSegmentId, and aligningSegmentIds from global store to local store
    useEffect(() => {
        const unsubscribePlayback = useTranscriptPlaybackStore.subscribe((state, prevState) => {
            const updates: Partial<TranscriptUIState> = {};
            let hasUpdates = false;

            if (state.activeSegmentId !== prevState.activeSegmentId) {
                updates.activeSegmentId = state.activeSegmentId;
                hasUpdates = true;
            }

            if (hasUpdates) {
                uiStore.setState(updates);
            }
        });

        const unsubscribeSession = useTranscriptSessionStore.subscribe((state, prevState) => {
            const updates: Partial<TranscriptUIState> = {};
            let hasUpdates = false;

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

        return () => {
            unsubscribePlayback();
            unsubscribeSession();
        };
    }, [uiStore]);

    const handleAnimationEnd = useCallback((id: string) => {
        knownSegmentIdsRef.current.add(id);

        // Update store directly to remove from newSegmentIds without triggering full re-render
        uiStore.setState((state) => {
            if (!state.newSegmentIds.has(id)) return state;

            const next = new Set(state.newSegmentIds);
            next.delete(id);
            return { newSegmentIds: next };
        });
    }, [uiStore]);

    return { uiStore, handleAnimationEnd };
}
