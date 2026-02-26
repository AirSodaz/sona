import React, { useRef, useCallback, useMemo, useEffect } from 'react';
import { useAutoScroll } from '../hooks/useAutoScroll';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { TranscriptSegment } from '../types/transcript';
import { PlusCircleIcon } from './Icons';
import { SegmentItem } from './transcript/SegmentItem';
import { TranscriptUIContext } from './transcript/TranscriptUIContext';
import { SearchUI } from './SearchUI';
import { EditorToolbar } from './EditorToolbar';
import { useSearchStore } from '../stores/searchStore';
import { useTranscriptUIState } from '../hooks/useTranscriptUIState';
import { useSegmentAlignment } from '../hooks/useSegmentAlignment';

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

    // Hooks for UI state and alignment
    const { uiStore, handleAnimationEnd } = useTranscriptUIState(segments);
    const requestAlignment = useSegmentAlignment();

    // Keep a ref to segments to make callbacks stable where needed
    const segmentsRef = useRef(segments);
    segmentsRef.current = segments;

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
            <EditorToolbar />
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
