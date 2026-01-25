import React, { useRef, useCallback, useEffect } from 'react';
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

const SegmentItem: React.FC<SegmentItemProps> = ({
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
            // Don't select all textual content by default, just focus at end or start is usually better UX 
            // but keeping select() if that was intended behavior, or better:
            inputRef.current.setSelectionRange(inputRef.current.value.length, inputRef.current.value.length);
            // inputRef.current.select(); // Original behavior
        }
    }, [isEditing]);

    useEffect(() => {
        setEditText(segment.text);
    }, [segment.text]);

    const handleTimestampClick = (e: React.MouseEvent) => {
        e.stopPropagation();
        onSeek(segment.start);
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
                data-tooltip="Click to seek"
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
                        data-tooltip="Merge with next"
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
                    data-tooltip="Delete segment"
                >
                    <TrashIcon />
                </button>
            </div>
        </div>
    );
};

interface TranscriptEditorProps {
    onSeek?: (time: number) => void;
}

export const TranscriptEditor: React.FC<TranscriptEditorProps> = ({ onSeek }) => {
    const virtuosoRef = useRef<VirtuosoHandle>(null);

    const segments = useTranscriptStore((state) => state.segments);
    const activeSegmentId = useTranscriptStore((state) => state.activeSegmentId);
    const editingSegmentId = useTranscriptStore((state) => state.editingSegmentId);
    const isPlaying = useTranscriptStore((state) => state.isPlaying);
    const updateSegment = useTranscriptStore((state) => state.updateSegment);
    const deleteSegment = useTranscriptStore((state) => state.deleteSegment);
    const mergeSegments = useTranscriptStore((state) => state.mergeSegments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);

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
        const index = segments.findIndex((s) => s.id === id);
        if (index !== -1 && index < segments.length - 1) {
            mergeSegments(id, segments[index + 1].id);
        }
    }, [segments, mergeSegments]);

    if (segments.length === 0) {
        return (
            <div className="empty-state">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M9 12h6M12 9v6M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
                </svg>
                <p>No transcript segments yet.<br />Start recording or import a file.</p>
            </div>
        );
    }

    return (
        <div className="transcript-editor">
            <Virtuoso
                ref={virtuosoRef}
                className="transcript-list"
                data={segments}
                itemContent={(index, segment) => (
                    <SegmentItem
                        key={segment.id}
                        segment={segment}
                        isActive={segment.id === activeSegmentId}
                        isEditing={segment.id === editingSegmentId}
                        onSeek={handleSeek}
                        onEdit={handleEdit}
                        onSave={handleSave}
                        onDelete={handleDelete}
                        onMergeWithNext={handleMergeWithNext}
                        hasNext={index < segments.length - 1}
                    />
                )}
            />
        </div>
    );
};

export default TranscriptEditor;
