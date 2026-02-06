import React, { useRef, useCallback, useMemo, useState } from 'react';
import { useAutoScroll } from '../hooks/useAutoScroll';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { TranscriptSegment } from '../types/transcript';
import { PlusCircleIcon } from './Icons';
import { SegmentItem } from './transcript/SegmentItem';


/** Context passed to virtualized list items. */
interface TranscriptContext {
    isLast: (index: number) => boolean;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    onAnimationEnd: (id: string) => void;
    newSegmentIds: Set<string>;
}

/** Props for TranscriptEditor. */
interface TranscriptEditorProps {
    /** Callback fired when a segment requests to seek to a timestamp. */
    onSeek?: (time: number) => void;
}

/**
 * Editor component for displaying and managing transcript segments.
 *
 * Uses virtualization for performance with large transcripts.
 *
 * @param props Component props.
 * @return The transcript editor interface.
 */
export function TranscriptEditor({ onSeek }: TranscriptEditorProps): React.JSX.Element {
    const { t } = useTranslation();
    const { confirm } = useDialogStore();
    const virtuosoRef = useRef<VirtuosoHandle>(null);

    const segments = useTranscriptStore((state) => state.segments);
    const updateSegment = useTranscriptStore((state) => state.updateSegment);
    const deleteSegment = useTranscriptStore((state) => state.deleteSegment);
    const mergeSegments = useTranscriptStore((state) => state.mergeSegments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);

    // Track which segment IDs have been seen (for animation)
    const knownSegmentIdsRef = useRef<Set<string>>(new Set());
    const prevNewSegmentIdsRef = useRef<Set<string>>(new Set());
    const [animationVersion, setAnimationVersion] = useState(0);

    // Compute new segment IDs synchronously during render so the segment-new
    // class is applied on the first render (CSS animations require the class
    // to be present when the element mounts).
    const newSegmentIds = useMemo(() => {
        const known = knownSegmentIdsRef.current;
        const newIds = new Set<string>();
        let hasNew = false;

        for (const segment of segments) {
            if (!known.has(segment.id)) {
                newIds.add(segment.id);
                hasNew = true;
            }
        }

        // Optimization: Return stable reference if the set of new IDs hasn't changed.
        // This is critical for preventing Virtuoso context updates (and full re-renders)
        // when segments are merely updated (e.g. text edit) rather than added.
        const prev = prevNewSegmentIdsRef.current;

        // Fast path: both empty
        if (!hasNew && prev.size === 0) {
            return prev;
        }

        // Slow path: check for equality
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

    // Keep a ref to segments to make callbacks stable
    const segmentsRef = useRef(segments);
    // Update ref in render body to ensure it's available for itemContent in the same render cycle
    segmentsRef.current = segments;

    // Auto-scroll to active segment during playback
    useAutoScroll(virtuosoRef);

    const handleSeek = useCallback((time: number) => {
        onSeek?.(time);
    }, [onSeek]);

    const handleEdit = useCallback((id: string) => {
        setEditingSegmentId(id);
    }, [setEditingSegmentId]);

    const handleSave = useCallback((id: string, text: string) => {
        updateSegment(id, { text });
        setEditingSegmentId(null);
    }, [updateSegment, setEditingSegmentId]);

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

    const contextValue = useMemo<TranscriptContext>(() => ({
        isLast: (index: number) => index === segmentsRef.current.length - 1,
        onSeek: handleSeek,
        onEdit: handleEdit,
        onSave: handleSave,
        onDelete: handleDelete,
        onMergeWithNext: handleMergeWithNext,
        onAnimationEnd: handleAnimationEnd,
        newSegmentIds,
    }), [handleSeek, handleEdit, handleSave, handleDelete, handleMergeWithNext, handleAnimationEnd, newSegmentIds]);

    const itemContent = useCallback((index: number, segment: TranscriptSegment, context: TranscriptContext) => (
        <SegmentItem
            key={segment.id}
            segment={segment}
            onSeek={context.onSeek}
            onEdit={context.onEdit}
            onSave={context.onSave}
            onDelete={context.onDelete}
            onMergeWithNext={context.onMergeWithNext}
            onAnimationEnd={context.onAnimationEnd}
            hasNext={!context.isLast(index)}
            isNew={context.newSegmentIds.has(segment.id)}
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
            <Virtuoso<TranscriptSegment, TranscriptContext>
                ref={virtuosoRef}
                className="transcript-list"
                data={segments}
                context={contextValue}
                itemContent={itemContent}
            />
        </div>
    );
}

export default TranscriptEditor;
