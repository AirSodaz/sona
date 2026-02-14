import React, { useRef, useCallback, useMemo, useState, useLayoutEffect, useEffect } from 'react';
import { useAutoScroll } from '../hooks/useAutoScroll';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { TranscriptSegment } from '../types/transcript';
import { PlusCircleIcon } from './Icons';
import { SegmentItem } from './transcript/SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from './transcript/TranscriptUIContext';
import { SearchUI } from './SearchUI';
import { useSearchStore } from '../stores/searchStore';


/** Context passed to virtualized list items via Virtuoso. */
interface TranscriptContext {
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    onAnimationEnd: (id: string) => void;
}

/** Props for TranscriptEditor. */
interface TranscriptEditorProps {
    // No props currently
}

/**
 * Editor component for displaying and managing transcript segments.
 *
 * Uses virtualization for performance with large transcripts.
 * Optimized to minimize re-renders during high-frequency updates.
 *
 * @return The transcript editor interface.
 * @param _props
 */
export function TranscriptEditor(_props: TranscriptEditorProps): React.JSX.Element {
    const { t } = useTranslation();
    const { confirm } = useDialogStore();
    const virtuosoRef = useRef<VirtuosoHandle>(null);

    const segments = useTranscriptStore((state) => state.segments);
    const updateSegment = useTranscriptStore((state) => state.updateSegment);
    const deleteSegment = useTranscriptStore((state) => state.deleteSegment);
    const mergeSegments = useTranscriptStore((state) => state.mergeSegments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);
    const requestSeek = useTranscriptStore((state) => state.requestSeek);

    // Track which segment IDs have been seen (for animation)
    const knownSegmentIdsRef = useRef<Set<string>>(new Set());
    const prevNewSegmentIdsRef = useRef<Set<string>>(new Set());
    const [animationVersion, setAnimationVersion] = useState(0);

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
    }, [segments, animationVersion]);

    // Sync newSegmentIds and totalSegments to local store
    useLayoutEffect(() => {
        uiStore.setState({
            newSegmentIds,
            totalSegments: segments.length
        });
    }, [newSegmentIds, segments.length, uiStore]);

    // Sync activeSegmentId, editingSegmentId, and aligningSegmentIds from global store to local store
    // This avoids this component re-rendering when they change
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

    // Keep a ref to segments to make callbacks stable
    const segmentsRef = useRef(segments);
    segmentsRef.current = segments;

    // Debounced alignment timers per segment
    const alignTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

    // Cleanup alignment timers on unmount
    useEffect(() => {
        return () => {
            for (const timer of alignTimersRef.current.values()) {
                clearTimeout(timer);
            }
            alignTimersRef.current.clear();
        };
    }, []);

    /**
     * Requests CTC re-alignment for a segment after a debounce period.
     * Spawns the sidecar to produce fresh tokens/timestamps/durations.
     */
    const requestAlignment = useCallback((segmentId: string) => {
        // Cancel any pending alignment for this segment
        const existing = alignTimersRef.current.get(segmentId);
        if (existing) {
            clearTimeout(existing);
        }

        const timer = setTimeout(async () => {
            alignTimersRef.current.delete(segmentId);

            // Re-read segment from store (may have been edited again or deleted)
            const segment = useTranscriptStore.getState().segments.find(s => s.id === segmentId);
            if (!segment) return;

            const store = useTranscriptStore.getState();
            store.addAligningSegmentId(segmentId);

            try {
                const result = await transcriptionService.alignSegment(segment);
                // Verify segment still exists before applying
                const current = useTranscriptStore.getState().segments.find(s => s.id === segmentId);
                if (current && result) {
                    useTranscriptStore.getState().updateSegment(segmentId, {
                        tokens: result.tokens,
                        timestamps: result.timestamps,
                        durations: result.durations,
                    });
                }
            } catch (error) {
                console.error('[TranscriptEditor] Alignment failed:', error);
            } finally {
                useTranscriptStore.getState().removeAligningSegmentId(segmentId);
            }
        }, 1500);

        alignTimersRef.current.set(segmentId, timer);
    }, []);

    // Auto-scroll to active segment during playback
    useAutoScroll(virtuosoRef);

    const handleSeek = useCallback((time: number) => {
        requestSeek(time);
    }, [requestSeek]);

    const handleEdit = useCallback((id: string) => {
        setEditingSegmentId(id);
    }, [setEditingSegmentId]);

    const handleSave = useCallback((id: string, text: string) => {
        // Check if text actually changed and alignment is possible
        const segment = segmentsRef.current.find(s => s.id === id);
        const textChanged = segment && segment.text !== text;

        updateSegment(id, { text });
        setEditingSegmentId(null);

        // Trigger re-alignment if text changed and segment has token data
        if (textChanged && segment.tokens && segment.tokens.length > 0) {
            const config = useTranscriptStore.getState().config;
            if (config.ctcModelPath) {
                requestAlignment(id);
            }
        }
    }, [updateSegment, setEditingSegmentId, requestAlignment]);

    const handleDelete = useCallback(async (id: string) => {
        const confirmed = await confirm(t('editor.delete_confirm_message', { defaultValue: 'Are you sure you want to delete this segment?' }), {
            title: t('editor.delete_confirm_title', { defaultValue: 'Confirm Delete' }),
            variant: 'warning'
        });

        if (confirmed) {
            deleteSegment(id);
        }
    }, [deleteSegment, t, confirm]);

    const handleMergeWithNext = useCallback(async (id: string) => {
        const confirmed = await confirm(t('editor.merge_confirm_message', { defaultValue: 'Merge this segment with the next one?' }), {
            title: t('editor.merge_confirm_title', { defaultValue: 'Confirm Merge' }),
            variant: 'info'
        });

        if (confirmed) {
            const currentSegments = segmentsRef.current;
            const index = currentSegments.findIndex((s) => s.id === id);
            if (index !== -1 && index < currentSegments.length - 1) {
                mergeSegments(id, currentSegments[index + 1].id);
            }
        }
    }, [mergeSegments, t]);

    const handleAnimationEnd = useCallback((id: string) => {
        knownSegmentIdsRef.current.add(id);
        setAnimationVersion(v => v + 1); // Trigger useMemo recomputation
    }, []);

    // Stable context for Virtuoso items (callbacks only)
    const contextValue = useMemo<TranscriptContext>(() => ({
        onSeek: handleSeek,
        onEdit: handleEdit,
        onSave: handleSave,
        onDelete: handleDelete,
        onMergeWithNext: handleMergeWithNext,
        onAnimationEnd: handleAnimationEnd,
    }), [handleSeek, handleEdit, handleSave, handleDelete, handleMergeWithNext, handleAnimationEnd]);

    const itemContent = useCallback((index: number, segment: TranscriptSegment, context: TranscriptContext) => (
        <SegmentItem
            key={segment.id}
            segment={segment}
            index={index}
            onSeek={context.onSeek}
            onEdit={context.onEdit}
            onSave={context.onSave}
            onDelete={context.onDelete}
            onMergeWithNext={context.onMergeWithNext}
            onAnimationEnd={context.onAnimationEnd}
        />
    ), []);

    // Search integration
    const {
        isOpen: isSearchOpen,
        open: openSearch,
        matches: searchMatches,
        currentMatchIndex: searchMatchIndex
    } = useSearchStore();

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                e.preventDefault();
                openSearch();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [openSearch]);

    // Scroll to active match
    useEffect(() => {
        if (isSearchOpen && searchMatches.length > 0 && searchMatchIndex >= 0) {
            const match = searchMatches[searchMatchIndex];
            const segmentIndex = segmentsRef.current.findIndex(s => s.id === match.segmentId);

            if (segmentIndex !== -1 && virtuosoRef.current) {
                virtuosoRef.current.scrollToIndex({
                    index: segmentIndex,
                    align: 'center',
                    behavior: 'smooth'
                });
            }
        }
    }, [isSearchOpen, searchMatchIndex, searchMatches]);

    if (segments.length === 0) {
        return (
            <div className="empty-state">
                <PlusCircleIcon />
                <p dangerouslySetInnerHTML={{ __html: t('editor.empty_state') }} />
            </div>
        );
    }

    return (
        <div className="transcript-editor">
            <TranscriptUIContext.Provider value={uiStore}>
                <Virtuoso<TranscriptSegment, TranscriptContext>
                    ref={virtuosoRef}
                    className="transcript-list"
                    data={segments}
                    context={contextValue}
                    itemContent={itemContent}
                    components={{
                        Header: () => <div style={{ height: '60px' }} />,
                        Footer: () => <div style={{ height: '50vh' }} />
                    }}
                />
            </TranscriptUIContext.Provider>
            <SearchUI />
        </div>
    );
}

export default TranscriptEditor;
