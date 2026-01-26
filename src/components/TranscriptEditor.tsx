import React, { useRef, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';
import { TranscriptSegment } from '../types/transcript';
import { formatDisplayTime } from '../utils/exportFormats';

// Icons
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

interface TranscriptContext {
    activeSegmentId: string | null;
    editingSegmentId: string | null;
    totalSegments: number;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
}

interface SegmentItemProps {
    segment: TranscriptSegment;
    isActive: boolean;
    isEditing: boolean;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    hasNext: boolean;
}

const SegmentItem = React.memo<SegmentItemProps>(({
    segment,
    isActive,
    isEditing,
    onSeek,
    onEdit,
    onSave,
    onDelete,
    onMergeWithNext,
    hasNext,
}) => {
    const { t } = useTranslation();
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
                aria-label={`Seek to ${formatDisplayTime(segment.start)}`}
                data-tooltip={t('editor.seek_tooltip')}
            >
                {formatDisplayTime(segment.start)}
            </span>

            <div className="segment-content" onClick={handleTextClick}>
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
                {hasNext && (
                    <button
                        className="btn btn-icon"
                        onClick={(e) => {
                            e.stopPropagation();
                            onMergeWithNext(segment.id);
                        }}
                        data-tooltip={t('editor.merge_tooltip')}
                        aria-label="Merge with next segment"
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
                    aria-label="Delete segment"
                >
                    <TrashIcon />
                </button>
            </div>
        </div>
    );
});

interface TranscriptEditorProps {
    onSeek?: (time: number) => void;
}

export const TranscriptEditor: React.FC<TranscriptEditorProps> = ({ onSeek }) => {
    const { t } = useTranslation();
    const virtuosoRef = useRef<VirtuosoHandle>(null);

    const segments = useTranscriptStore((state) => state.segments);
    const activeSegmentId = useTranscriptStore((state) => state.activeSegmentId);
    const editingSegmentId = useTranscriptStore((state) => state.editingSegmentId);
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const updateSegment = useTranscriptStore((state) => state.updateSegment);
    const deleteSegment = useTranscriptStore((state) => state.deleteSegment);
    const mergeSegments = useTranscriptStore((state) => state.mergeSegments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);

    // Keep a ref to segments to make callbacks stable
    const segmentsRef = useRef(segments);
    useEffect(() => {
        segmentsRef.current = segments;
    }, [segments]);

    // Auto-scroll to active segment during playback
    useEffect(() => {
        if (isPlaying && activeSegmentId && virtuosoRef.current) {
            const activeIndex = segments.findIndex((s) => s.id === activeSegmentId);
            if (activeIndex !== -1) {
                virtuosoRef.current.scrollToIndex({
                    index: activeIndex,
                    align: 'center',
                    behavior: 'smooth',
                });
            }
        }
    }, [activeSegmentId, isPlaying, segments]);

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

    const handleDelete = useCallback((id: string) => {
        deleteSegment(id);
    }, [deleteSegment]);

    const handleMergeWithNext = useCallback((id: string) => {
        const currentSegments = segmentsRef.current;
        const index = currentSegments.findIndex((s) => s.id === id);
        if (index !== -1 && index < currentSegments.length - 1) {
            mergeSegments(id, currentSegments[index + 1].id);
        }
    }, [mergeSegments]);

    const contextValue = useMemo<TranscriptContext>(() => ({
        activeSegmentId,
        editingSegmentId,
        totalSegments: segments.length,
        onSeek: handleSeek,
        onEdit: handleEdit,
        onSave: handleSave,
        onDelete: handleDelete,
        onMergeWithNext: handleMergeWithNext,
    }), [activeSegmentId, editingSegmentId, segments.length, handleSeek, handleEdit, handleSave, handleDelete, handleMergeWithNext]);

    const itemContent = useCallback((index: number, segment: TranscriptSegment, context: TranscriptContext) => (
        <SegmentItem
            key={segment.id}
            segment={segment}
            isActive={segment.id === context.activeSegmentId}
            isEditing={segment.id === context.editingSegmentId}
            onSeek={context.onSeek}
            onEdit={context.onEdit}
            onSave={context.onSave}
            onDelete={context.onDelete}
            onMergeWithNext={context.onMergeWithNext}
            hasNext={index < context.totalSegments - 1}
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
};

export default TranscriptEditor;
