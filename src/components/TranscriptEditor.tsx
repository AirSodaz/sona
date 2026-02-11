import React, { useRef, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore } from '../stores/transcriptStore';
import { TranscriptSegment } from '../types/transcript';
import { PlusCircleIcon } from './Icons';
import { SegmentItem } from './transcript/SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from './transcript/TranscriptUIContext';
import { SearchUI } from './SearchUI';

import { useAutoScroll } from '../hooks/useAutoScroll';
import { useNewSegments } from '../hooks/useNewSegments';
import { useSegmentAlignment } from '../hooks/useSegmentAlignment';
import { useSearchShortcuts } from '../hooks/useSearchShortcuts';
import { useTranscriptActions } from '../hooks/useTranscriptActions';


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
 * @param props Component props.
 * @return The transcript editor interface.
 */
export function TranscriptEditor(_props: TranscriptEditorProps): React.JSX.Element {
    const { t } = useTranslation();
    const virtuosoRef = useRef<VirtuosoHandle>(null);

    const segments = useTranscriptStore((state) => state.segments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);
    const requestSeek = useTranscriptStore((state) => state.requestSeek);

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

    // Track new segments and sync UI state
    const { knownSegmentIdsRef } = useNewSegments(segments, uiStore, animationVersion);

    // Handle segment alignment (CTC)
    const requestAlignment = useSegmentAlignment();

    // Keep a ref to segments to make callbacks stable
    const segmentsRef = useRef(segments);
    segmentsRef.current = segments;

    // Handle transcript actions (Save, Delete, Merge)
    const { handleSave, handleDelete, handleMergeWithNext } = useTranscriptActions({
        segmentsRef,
        requestAlignment
    });

    // Handle search shortcuts (Ctrl+F) and scrolling
    useSearchShortcuts(virtuosoRef, segmentsRef);

    // Auto-scroll to active segment during playback
    useAutoScroll(virtuosoRef);

    const handleSeek = useCallback((time: number) => {
        requestSeek(time);
    }, [requestSeek]);

    const handleEdit = useCallback((id: string) => {
        setEditingSegmentId(id);
    }, [setEditingSegmentId]);

    const handleAnimationEnd = useCallback((id: string) => {
        knownSegmentIdsRef.current.add(id);
        setAnimationVersion(v => v + 1); // Trigger useMemo recomputation
    }, [knownSegmentIdsRef]);

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
