import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { EditIcon, TrashIcon, MergeIcon } from '../Icons';
import { SegmentTimestamp } from './SegmentTimestamp';

/** Props for SegmentItem component. */
export interface SegmentItemProps {
    segment: TranscriptSegment;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    onAnimationEnd: (id: string) => void;
    hasNext: boolean;
    isNew: boolean;
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
    onAnimationEnd,
    hasNext,
    isNew,
}: SegmentItemProps): React.JSX.Element {
    const { t } = useTranslation();
    const isActive = useTranscriptStore(useCallback((state) => state.activeSegmentId === segment.id, [segment.id]));
    const isEditing = useTranscriptStore(useCallback((state) => state.editingSegmentId === segment.id, [segment.id]));
    const [editText, setEditText] = useState(segment.text);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    // Auto-resize textarea
    useLayoutEffect(() => {
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

    function handleAnimationEnd(e: React.AnimationEvent): void {
        // Only respond to our fade-in animation, not animations on child elements
        if (isNew && e.animationName === 'segmentFadeIn' && e.target === e.currentTarget) {
            onAnimationEnd(segment.id);
        }
    }

    const classNames = [
        'transcript-segment',
        isActive ? 'active' : '',
        isEditing ? 'editing' : '',
        isNew ? 'segment-new' : '',
    ].filter(Boolean).join(' ');

    return (
        <div className={classNames} onAnimationEnd={handleAnimationEnd}>
            <SegmentTimestamp start={segment.start} onSeek={onSeek} />

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

export const SegmentItem = React.memo(SegmentItemComponent);
