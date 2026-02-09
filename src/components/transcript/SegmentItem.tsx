import React, { useCallback, useEffect, useLayoutEffect, useRef, useState, useContext } from 'react';
import { useTranslation } from 'react-i18next';
import { useStore } from 'zustand';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { alignTokensToText } from '../../utils/segmentUtils';
import { EditIcon, TrashIcon, MergeIcon } from '../Icons';
import { SegmentTimestamp } from './SegmentTimestamp';
import { TranscriptUIContext } from './TranscriptUIContext';

/** Props for SegmentItem component. */
export interface SegmentItemProps {
    segment: TranscriptSegment;
    index: number;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    onAnimationEnd: (id: string) => void;
}

/**
 * Individual transcript segment item.
 * Supports viewing, seeking, editing, deleting, and merging.
 */
function SegmentItemComponent({
    segment,
    index,
    onSeek,
    onEdit,
    onSave,
    onDelete,
    onMergeWithNext,
    onAnimationEnd,
}: SegmentItemProps): React.JSX.Element {
    const { t } = useTranslation();

    // Subscribe to UI state via context store to avoid parent re-renders and global store noise
    const uiStore = useContext(TranscriptUIContext);
    if (!uiStore) throw new Error('SegmentItem must be used within TranscriptUIContext');

    const isActive = useStore(uiStore, useCallback((state) => state.activeSegmentId === segment.id, [segment.id]));
    const isEditing = useStore(uiStore, useCallback((state) => state.editingSegmentId === segment.id, [segment.id]));
    const isNew = useStore(uiStore, useCallback((state) => state.newSegmentIds.has(segment.id), [segment.id]));
    const currentTime = useStore(uiStore, useCallback((state) => state.activeSegmentId === segment.id ? state.currentTime : -1, [segment.id]));

    // Align tokens with formatted text
    const alignedTokens = React.useMemo(() => {
        if (!segment.tokens || !segment.timestamps) return null;
        const result = alignTokensToText(segment.text, segment.tokens, segment.timestamps);
        // console.log('SegmentItem alignment:', { 
        //     id: segment.id,
        //     text: segment.text, 
        //     firstToken: segment.tokens[0], 
        //     alignedCount: result.length 
        // });
        return result;
    }, [segment.text, segment.tokens, segment.timestamps]);

    // Determine active token index if segment is active
    const activeTokenIndex = React.useMemo(() => {
        if (!isActive || !alignedTokens || currentTime < 0) return -1;
        // Find the last token that has started
        const nextTokenIndex = alignedTokens.findIndex(t => t.timestamp > currentTime);
        return nextTokenIndex === -1 ? alignedTokens.length - 1 : nextTokenIndex - 1;
    }, [isActive, alignedTokens, currentTime]);
    // Subscribe to store for hasNext to avoid passing unstable props
    const hasNext = useStore(uiStore, useCallback((state) => index < state.totalSegments - 1, [index]));

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
                        {alignedTokens ? (
                            alignedTokens.map((tokenObj, i) => (
                                <span
                                    key={i}
                                    title={formatDisplayTime(tokenObj.timestamp)}
                                    className={`token-hover ${i === activeTokenIndex ? 'active-token' : ''}`}
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        onSeek(tokenObj.timestamp);
                                    }}
                                >
                                    {tokenObj.text}
                                </span>
                            ))
                        ) : (
                            segment.text || '(empty)'
                        )}
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
