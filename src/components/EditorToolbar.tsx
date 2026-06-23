import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { splitTranscriptSegment } from '../stores/transcriptCoordinator';
import { getActiveEditor } from '../stores/transcriptRuntimeStore';
import { serializeSplitBlocks } from '../utils/lexicalSplitUtils';
import {
  FORMAT_TEXT_COMMAND,
  UNDO_COMMAND,
  REDO_COMMAND,
  type LexicalEditor,
} from 'lexical';
import {
    UndoIcon,
    RedoIcon,
    BoldIcon,
    ItalicIcon,
    UnderlineIcon,
    ReturnIcon
} from './Icons';

const SAVED_STATUS_VISIBLE_MS = 1500;

function handleToolbarSplit(editor: LexicalEditor, segmentId: string): void {
  const result = serializeSplitBlocks(editor);
  if (result) {
    splitTranscriptSegment(segmentId, result.leftHtml, result.rightHtml);
  }
}

export function EditorToolbar(): React.JSX.Element | null {
    const { t } = useTranslation();
    const editingSegmentId = useTranscriptSessionStore((state) => state.editingSegmentId);
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const autoSaveState = useTranscriptSidecarStore((state) => (
        sourceHistoryId ? state.autoSaveStates[sourceHistoryId] : undefined
    ));

    const isEditing = Boolean(editingSegmentId);
    const [isSavedVisible, setIsSavedVisible] = useState(false);

    const handleAction = (command: string) => {
        const editor = getActiveEditor();
        if (!editor) return;

        switch (command) {
            case 'undo':
                editor.dispatchCommand(UNDO_COMMAND, undefined);
                break;
            case 'redo':
                editor.dispatchCommand(REDO_COMMAND, undefined);
                break;
            case 'bold':
                editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'bold');
                break;
            case 'italic':
                editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'italic');
                break;
            case 'underline':
                editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'underline');
                break;
            case 'insertLineBreak':
                if (editingSegmentId) {
                    handleToolbarSplit(editor, editingSegmentId);
                }
                break;
        }
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
                        data-tooltip={t('editor.split_segment', 'Split segment')}
                        data-tooltip-pos="top"
                        aria-label={t('editor.split_segment', 'Split segment')}
                    >
                        <ReturnIcon />
                    </button>
                </div>
            )}
        </>
    );
}
