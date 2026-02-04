import React, { useRef, useCallback, useEffect, useMemo } from 'react';
import { useAutoScroll } from '../hooks/useAutoScroll';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { TranscriptSegment } from '../types/transcript';
import { formatDisplayTime } from '../utils/exportFormats';
import { EditIcon, TrashIcon, MergeIcon } from './Icons';

// Icons


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

    function handleTimestampClick(e: React.MouseEvent): void {
        e.stopPropagation();
        onSeek(segment.start);
    }

    function handleTimestampKeyDown(e: React.KeyboardEvent): void {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            onSeek(segment.start);
        }
    }

    function handleTextClick(): void {
        if (!isEditing) {
            onSeek(segment.start);
        }
    }

    function handleTextDoubleClick(e: React.MouseEvent): void {
        if (!isEditing) {
            e.stopPropagation();
            onEdit(segment.id);
        }
    }

    function handleKeyDown(e: React.KeyboardEvent): void {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            onSave(segment.id, editText);
        } else if (e.key === 'Escape') {
            setEditText(segment.text);
            onSave(segment.id, segment.text);
        }
    }

    function handleBlur(): void {
        onSave(segment.id, editText);
    }

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
