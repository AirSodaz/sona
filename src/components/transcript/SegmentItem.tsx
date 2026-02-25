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
import { useTranscriptStore } from '../../stores/transcriptStore';

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

function textToHtml(text: string): string {
    if (!text) return '';
    return text.replace(/\n/g, '<br>');
}

function htmlToText(html: string): string {
    if (!html) return '';

    // Replace block elements and breaks with newlines
    let text = html
        .replace(/<div>/gi, '\n')
        .replace(/<\/div>/gi, '')
        .replace(/<p>/gi, '\n')
        .replace(/<\/p>/gi, '')
        .replace(/<br\s*\/?>/gi, '\n');

    // Normalize formatting tags
    text = text
        .replace(/<(\/?)strong/gi, '<$1b')
        .replace(/<(\/?)em/gi, '<$1i');

    // Normalize spaces
    text = text.replace(/&nbsp;/g, ' ');

    // Strip all tags EXCEPT b, i, u
    // Regex explanation: Match <...> where content does NOT start with /?(b|i|u) followed by > or space
    text = text.replace(/<(?!\/?(?:b|i|u)(?:>|\s))[^>]*>/gi, '');

    return text;
}

const ContentEditable = React.forwardRef<HTMLDivElement, {
    html: string;
    onChange: (e: React.FormEvent<HTMLDivElement>) => void;
    onBlur: () => void;
    onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
    className?: string;
}>(({ html, onChange, onBlur, onKeyDown, className }, ref) => {
    const divRef = useRef<HTMLDivElement>(null);

    // Sync html prop to div
    useLayoutEffect(() => {
        if (divRef.current && divRef.current.innerHTML !== html) {
             divRef.current.innerHTML = html;
        }

        if (typeof ref === 'function') {
            ref(divRef.current);
        } else if (ref) {
            ref.current = divRef.current;
        }
    }, [html, ref]);

    const handlePaste = (e: React.ClipboardEvent) => {
        e.preventDefault();
        const text = e.clipboardData.getData('text/plain');
        // Insert plain text (strips formatting from source to be safe,
        // or we could sanitize HTML paste, but plain text is safer default)
        document.execCommand('insertText', false, text);
    };

    return (
        <div
            ref={divRef}
            className={className}
            contentEditable
            onInput={onChange}
            onBlur={onBlur}
            onKeyDown={onKeyDown}
            onPaste={handlePaste}
            style={{
                whiteSpace: 'pre-wrap',
                overflowY: 'auto',
                // Mimic textarea styles from css if needed, but class should handle most
                minHeight: '1.8em',
                outline: 'none'
            }}
        />
    );
});
ContentEditable.displayName = 'ContentEditable';

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

    // Translation visibility
    const isTranslationVisible = useTranscriptStore(state => state.isTranslationVisible);

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

    // Local state stores HTML for the editor
    const [editText, setEditText] = useState(() => textToHtml(segment.text));
    const inputRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (isEditing && inputRef.current) {
            inputRef.current.focus();
            // Optional: Move cursor to end? ContentEditable logic makes this tricky without selection API.
            // But browser often focuses at start.
        }
    }, [isEditing]);

    useEffect(() => {
        setEditText(textToHtml(segment.text));
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

    function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>): void {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            // Save current HTML converted to text
            onSave(segment.id, htmlToText(e.currentTarget.innerHTML));
        } else if (e.key === 'Escape') {
            setEditText(textToHtml(segment.text));
            // Save original (cancel)
            onSave(segment.id, segment.text);
        } else if ((e.ctrlKey || e.metaKey)) {
             const key = e.key.toLowerCase();
             if (['b', 'i', 'u'].includes(key)) {
                 e.preventDefault();
                 const command = key === 'b' ? 'bold' : key === 'i' ? 'italic' : 'underline';
                 document.execCommand(command);
             }
        }
    }

    function handleBlur(): void {
        // Use current state or ref? State updates onInput, so editText is up to date (mostly).
        // But safer to use htmlToText(editText)
        onSave(segment.id, htmlToText(editText));
    }

    function handleChange(e: React.FormEvent<HTMLDivElement>) {
        setEditText(e.currentTarget.innerHTML);
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
                    <ContentEditable
                        ref={inputRef}
                        className="segment-input"
                        html={editText}
                        onChange={handleChange}
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
                {isTranslationVisible && typeof segment.translation === 'string' && segment.translation && !isEditing && (
                    <div className="segment-translation" style={{ marginTop: '4px', color: 'var(--color-text-secondary)', fontSize: '0.9em' }}>
                        {segment.translation}
                    </div>
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
