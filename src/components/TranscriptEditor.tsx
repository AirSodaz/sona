import React, { useRef, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { TranscriptSegment } from '../types/transcript';
import { formatDisplayTime } from '../utils/exportFormats';

// Icons
const EditIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
        <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </svg>
);

const TrashIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M3 6h18" />
        <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
        <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
    </svg>
);

const MergeIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M8 6h13" />
        <path d="M8 12h13" />
        <path d="M8 18h13" />
        <path d="M3 6h.01" />
        <path d="M3 12h.01" />
        <path d="M3 18h.01" />
    </svg>
);

/** Context passed to virtualized list items. */
interface TranscriptContext {
    isLast: (index: number) => boolean;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
}

/** Props for SegmentItem component. */
interface SegmentItemProps {
    segment: TranscriptSegment;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    hasNext: boolean;
}

/**
 * Individual transcript segment item.
 * Supports viewing, seeking, editing, deleting, and merging.
 */
function SegmentItemComponent({
    segment,
    onSeek,
    onEdit,
    onSave,
    onDelete,
    onMergeWithNext,
    hasNext,
}: SegmentItemProps): React.JSX.Element {
    const { t } = useTranslation();
    const isActive = useTranscriptStore(useCallback((state) => state.activeSegmentId === segment.id, [segment.id]));
    const isEditing = useTranscriptStore(useCallback((state) => state.editingSegmentId === segment.id, [segment.id]));
    const [editText, setEditText] = React.useState(segment.text);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    // Auto-resize textarea
    React.useLayoutEffect(() => {
        if (isEditing && inputRef.current) {
            inputRef.current.style.height = 'auto';
            inputRef.current.style.height = `${inputRef.current.scrollHeight}px`;
        }
    }, [isEditing, editText]);

    useEffect(() => {
        if (isEditing && inputRef.current) {
            inputRef.current.focus();
            inputRef.current.setSelectionRange(inputRef.current.value.length, inputRef.current.value.length);
        }
    }, [isEditing]);

    useEffect(() => {
        setEditText(segment.text);
    }, [segment.text]);

    const handleTimestampClick = (e: React.MouseEvent) => {
        e.stopPropagation();
        onSeek(segment.start);
    };

    const handleTimestampKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            onSeek(segment.start);
        }
    };

    const handleTextClick = () => {
        if (!isEditing) {
            onSeek(segment.start);
        }
    };

    const handleTextDoubleClick = (e: React.MouseEvent) => {
        if (!isEditing) {
            e.stopPropagation();
            onEdit(segment.id);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            onSave(segment.id, editText);
        } else if (e.key === 'Escape') {
            setEditText(segment.text);
            onSave(segment.id, segment.text);
        }
    };

    const handleBlur = () => {
        onSave(segment.id, editText);
    };

    const classNames = [
        'transcript-segment',
        isActive ? 'active' : '',
        isEditing ? 'editing' : '',
    ].filter(Boolean).join(' ');

    return (
        <div className={classNames}>
            <span
                className="segment-timestamp"
                onClick={handleTimestampClick}
                onKeyDown={handleTimestampKeyDown}
                role="button"
                tabIndex={0}
                aria-label={t('editor.seek_label', { time: formatDisplayTime(segment.start) })}
                data-tooltip={t('editor.seek_tooltip')}
            >
                {formatDisplayTime(segment.start)}
            </span>

            <div className="segment-content" onClick={handleTextClick} onDoubleClick={handleTextDoubleClick}>
                {isEditing ? (
                    <textarea
                        ref={inputRef}
                        className="segment-input"
                        value={editText}
                        onChange={(e) => setEditText(e.target.value)}
                        onKeyDown={handleKeyDown}
                        onBlur={handleBlur}
                    />
                ) : (
                    <p className={`segment-text ${!segment.isFinal ? 'partial' : ''}`}>
                        {segment.text || '(empty)'}
                    </p>
                )}
            </div>

            <div className="segment-actions">
                <button
                    className="btn btn-icon"
                    onClick={(e) => {
                        e.stopPropagation();
                        onEdit(segment.id);
                    }}
                    data-tooltip={t('editor.edit_tooltip')}
                    aria-label={t('editor.edit_label', { time: formatDisplayTime(segment.start) })}
                >
                    <EditIcon />
                </button>
                {hasNext && (
                    <button
                        className="btn btn-icon"
                        onClick={(e) => {
                            e.stopPropagation();
                            onMergeWithNext(segment.id);
                        }}
                        data-tooltip={t('editor.merge_tooltip')}
                        aria-label={t('editor.merge_label', { time: formatDisplayTime(segment.start) })}
                    >
                        <MergeIcon />
                    </button>
                )}
                <button
                    className="btn btn-icon"
                    onClick={(e) => {
                        e.stopPropagation();
                        onDelete(segment.id);
                    }}
                    data-tooltip={t('editor.delete_tooltip')}
                    aria-label={t('editor.delete_label', { time: formatDisplayTime(segment.start) })}
                >
                    <TrashIcon />
                </button>
            </div>
        </div>
    );
}
const SegmentItem = React.memo(SegmentItemComponent);

/** Props for TranscriptEditor. */
interface TranscriptEditorProps {
    /** Callback fired when a segment requests to seek to a timestamp. */
    onSeek?: (time: number) => void;
}

/**
 * Editor component for displaying and managing transcript segments.
 * Uses virtualization for performance with large transcripts.
 *
 * @param props - Component props.
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

    // Keep a ref to segments to make callbacks stable
    const segmentsRef = useRef(segments);
    // Update ref in render body to ensure it's available for itemContent in the same render cycle
    segmentsRef.current = segments;

    const lastActiveIndexRef = useRef<number>(-1);

    // Auto-scroll to active segment during playback
    // Using subscribe to avoid re-rendering the component on every segment change
    useEffect(() => {
        const unsub = useTranscriptStore.subscribe((state, prevState) => {
            const { activeSegmentId, isPlaying, segments } = state;
            const prevActiveId = prevState.activeSegmentId;
            const prevIsPlaying = prevState.isPlaying;

            // Only scroll if activeSegmentId changed OR isPlaying became true
            const shouldScroll = (activeSegmentId !== prevActiveId && activeSegmentId) ||
                (isPlaying && !prevIsPlaying && activeSegmentId);

            if (shouldScroll && isPlaying && virtuosoRef.current) {
                let activeIndex = -1;

                // Optimization: Check near the last known index first (O(1) for sequential playback)
                const lastIndex = lastActiveIndexRef.current;
                if (lastIndex >= 0 && lastIndex < segments.length) {
                    if (segments[lastIndex].id === activeSegmentId) {
                        activeIndex = lastIndex;
                    } else if (lastIndex + 1 < segments.length && segments[lastIndex + 1].id === activeSegmentId) {
                        activeIndex = lastIndex + 1;
                    }
                }

                // Fallback to full search if not found (O(N))
                if (activeIndex === -1) {
                    activeIndex = segments.findIndex((s) => s.id === activeSegmentId);
                }

                if (activeIndex !== -1) {
                    lastActiveIndexRef.current = activeIndex;
                    virtuosoRef.current.scrollToIndex({
                        index: activeIndex,
                        align: 'center',
                        behavior: 'smooth',
                    });
                }
            }
        });
        return unsub;
    }, []);

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

    const contextValue = useMemo<TranscriptContext>(() => ({
        isLast: (index: number) => index === segmentsRef.current.length - 1,
        onSeek: handleSeek,
        onEdit: handleEdit,
        onSave: handleSave,
        onDelete: handleDelete,
        onMergeWithNext: handleMergeWithNext,
    }), [handleSeek, handleEdit, handleSave, handleDelete, handleMergeWithNext]);

    const itemContent = useCallback((index: number, segment: TranscriptSegment, context: TranscriptContext) => (
        <SegmentItem
            key={segment.id}
            segment={segment}
            onSeek={context.onSeek}
            onEdit={context.onEdit}
            onSave={context.onSave}
            onDelete={context.onDelete}
            onMergeWithNext={context.onMergeWithNext}
            hasNext={!context.isLast(index)}
        />
    ), []);

    if (segments.length === 0) {
        return (
            <div className="empty-state">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 12h6M12 9v6M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
                </svg>
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
