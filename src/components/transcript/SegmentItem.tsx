import React, { useCallback, useEffect, useLayoutEffect, useRef, useState, useContext } from 'react';
import { useTranslation } from 'react-i18next';
import { useStore } from 'zustand';
import { useShallow } from 'zustand/react/shallow';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { EditIcon, TrashIcon, MergeIcon } from '../Icons';
import { SegmentTimestamp } from './SegmentTimestamp';
import { SegmentTokens } from './SegmentTokens';
import { TranscriptUIContext } from './TranscriptUIContext';
import { useSearchStore } from '../../stores/searchStore';

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
    const isAligning = useStore(uiStore, useCallback((state) => state.aligningSegmentIds.has(segment.id), [segment.id]));

    // Subscribe to store for hasNext to avoid passing unstable props
    const hasNext = useStore(uiStore, useCallback((state) => index < state.totalSegments - 1, [index]));

    // Search matches
    // Optimize: Select only what we need to avoid re-renders on every store change
    const matches = useSearchStore(useShallow(state =>
        state.matches.filter(m => m.segmentId === segment.id)
    ));
    const setActiveMatch = useSearchStore(useCallback(state => state.setActiveMatch, []));

    // Select active match only if it belongs to this segment
    // This prevents re-renders when the active match changes but is in a different segment
    const activeMatch = useSearchStore(useCallback((state) => {
        const match = state.matches[state.currentMatchIndex];
        return (match && match.segmentId === segment.id) ? match : null;
    }, [segment.id]));

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
                    <SegmentTokens
                        segment={segment}
                        isActive={isActive}
                        onSeek={onSeek}
                        matches={matches}
                        activeMatch={activeMatch}
                        onMatchClick={setActiveMatch}
                    />
                )}
                {isAligning && (
                    <span
                        className="segment-aligning-indicator"
                        data-tooltip={t('editor.aligning')}
                        aria-label={t('editor.aligning')}
                    />
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
