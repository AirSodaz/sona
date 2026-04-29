import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import {
    UndoIcon,
    RedoIcon,
    BoldIcon,
    ItalicIcon,
    UnderlineIcon,
    ReturnIcon
} from './Icons';

const SAVED_STATUS_VISIBLE_MS = 1500;

export function EditorToolbar(): React.JSX.Element | null {
    const { t } = useTranslation();
    const editingSegmentId = useTranscriptStore((state) => state.editingSegmentId);
    const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
    const autoSaveState = useTranscriptStore((state) => (
        state.sourceHistoryId ? state.autoSaveStates[state.sourceHistoryId] : undefined
    ));

    const isEditing = Boolean(editingSegmentId);
    const [isSavedVisible, setIsSavedVisible] = useState(false);

    const handleAction = (command: string, value?: string) => {
        document.execCommand(command, false, value);
    };

    // Prevent button click from stealing focus from the editable element
    const handleMouseDown = (e: React.MouseEvent) => {
        e.preventDefault();
    };

    useEffect(() => {
        if (!sourceHistoryId || autoSaveState?.status !== 'saved') {
            queueMicrotask(() => setIsSavedVisible(false));
            return undefined;
        }

        const elapsed = Date.now() - autoSaveState.updatedAt;
        if (elapsed >= SAVED_STATUS_VISIBLE_MS) {
            queueMicrotask(() => setIsSavedVisible(false));
            return undefined;
        }

        queueMicrotask(() => setIsSavedVisible(true));
        const timeout = window.setTimeout(() => {
            setIsSavedVisible(false);
        }, SAVED_STATUS_VISIBLE_MS - elapsed);

        return () => window.clearTimeout(timeout);
    }, [autoSaveState?.status, autoSaveState?.updatedAt, sourceHistoryId]);

    let saveStatus = null;
    if (sourceHistoryId && autoSaveState?.status) {
        if (autoSaveState.status === 'saved') {
            saveStatus = isSavedVisible ? 'saved' : null;
        } else {
            saveStatus = autoSaveState.status;
        }
    }

    if (!isEditing && !saveStatus) return null;

    const saveStatusLabel = saveStatus
        ? ({
            saving: t('editor.autosave_saving', 'Saving...'),
            saved: t('editor.autosave_saved', 'Saved'),
            error: t('editor.autosave_error', 'Save failed'),
        }[saveStatus])
        : null;

    return (
        <>
            {saveStatus && saveStatusLabel && (
                <div
                    className={`editor-autosave-status is-${saveStatus}`}
                    role="status"
                    aria-live="polite"
                >
                    {saveStatusLabel}
                </div>
            )}

            {isEditing && (
                <div className="editor-toolbar">
                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('undo')}
                        data-tooltip={t('editor.undo', 'Undo')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.undo', 'Undo')}
                    >
                        <UndoIcon />
                    </button>
                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('redo')}
                        data-tooltip={t('editor.redo', 'Redo')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.redo', 'Redo')}
                    >
                        <RedoIcon />
                    </button>

                    <div className="toolbar-divider" />

                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('bold')}
                        data-tooltip={t('editor.bold', 'Bold')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.bold', 'Bold')}
                    >
                        <BoldIcon />
                    </button>
                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('italic')}
                        data-tooltip={t('editor.italic', 'Italic')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.italic', 'Italic')}
                    >
                        <ItalicIcon />
                    </button>
                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('underline')}
                        data-tooltip={t('editor.underline', 'Underline')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.underline', 'Underline')}
                    >
                        <UnderlineIcon />
                    </button>

                    <div className="toolbar-divider" />

                    <button
                        className="btn-icon toolbar-btn"
                        onMouseDown={handleMouseDown}
                        onClick={() => handleAction('insertLineBreak')}
                        data-tooltip={t('editor.line_break', 'Line break')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.line_break', 'Line break')}
                    >
                        <ReturnIcon />
                    </button>
                </div>
            )}
        </>
    );
}
