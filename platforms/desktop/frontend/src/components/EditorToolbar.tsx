import { FORMAT_TEXT_COMMAND, REDO_COMMAND, UNDO_COMMAND } from 'lexical';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useBoundSessionId } from '../contexts/TranscriptSessionContext';
import {
  selectIsDocumentEditorLocked,
  useDocumentMutationLockStore,
} from '../services/task/documentMutationCoordinator';
import { getActiveEditor } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { serializeSplitBlocks } from '../utils/lexicalSplitUtils';
import {
  BoldIcon,
  CodeIcon,
  ItalicIcon,
  RedoIcon,
  ReturnIcon,
  StrikethroughIcon,
  UnderlineIcon,
  UndoIcon,
} from './Icons';
import { IconButton } from './ui/Button';

const SAVED_STATUS_VISIBLE_MS = 1500;

export function EditorToolbar(): React.JSX.Element | null {
  const { t } = useTranslation();
  const boundSessionId = useBoundSessionId();
  const editingSegmentId = useTranscriptSessionStore((state) => state.editingSegmentId);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const splitTranscriptSegment = useTranscriptSessionStore((state) => state.splitTranscriptSegment);
  const isDocumentLocked = useDocumentMutationLockStore((state) =>
    selectIsDocumentEditorLocked(state.locks, boundSessionId, sourceHistoryId)
  );
  const autoSaveState = useTranscriptSidecarStore((state) =>
    sourceHistoryId ? state.autoSaveStates[sourceHistoryId] : undefined
  );

  const isEditing = Boolean(editingSegmentId) && !isDocumentLocked;
  const [isSavedVisible, setIsSavedVisible] = useState(false);

  const handleAction = (command: string) => {
    if (isDocumentLocked) return;
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
      case 'strikethrough':
        editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'strikethrough');
        break;
      case 'code':
        editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'code');
        break;
      case 'insertLineBreak':
        if (editingSegmentId) {
          const result = serializeSplitBlocks(editor);
          if (result) {
            splitTranscriptSegment(editingSegmentId, result.leftHtml, result.rightHtml);
          }
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
    ? {
        saving: t('editor.autosave_saving', 'Saving...'),
        saved: t('editor.autosave_saved', 'Saved'),
        error: t('editor.autosave_error', 'Save failed'),
      }[saveStatus]
    : null;

  return (
    <>
      {saveStatus && saveStatusLabel && (
        <div
          className={`editor-autosave-status is-${saveStatus}${saveStatus === 'error' ? ' has-tooltip' : ''}`}
          role="status"
          aria-live="polite"
          tabIndex={saveStatus === 'error' ? 0 : undefined}
        >
          <span className="editor-autosave-status-label">{saveStatusLabel}</span>
          {saveStatus === 'error' && (
            <div className="editor-autosave-tooltip" role="tooltip">
              <div className="editor-autosave-tooltip-header">
                <span className="editor-autosave-tooltip-dot" />
                <span>{saveStatusLabel}</span>
              </div>
              <div className="editor-autosave-tooltip-message">
                {autoSaveState?.errorMessage ||
                  t('editor.autosave_error_default', 'Failed to save changes')}
              </div>
            </div>
          )}
        </div>
      )}

      {isEditing && (
        <div className="editor-toolbar">
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<UndoIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('undo')}
            tooltipPos="top"
            aria-label={t('editor.undo', 'Undo')}
          />
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<RedoIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('redo')}
            tooltipPos="top"
            aria-label={t('editor.redo', 'Redo')}
          />

          <div className="toolbar-divider" />

          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<BoldIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('bold')}
            tooltipPos="top"
            aria-label={t('editor.bold', 'Bold')}
          />
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<ItalicIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('italic')}
            tooltipPos="top"
            aria-label={t('editor.italic', 'Italic')}
          />
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<UnderlineIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('underline')}
            tooltipPos="top"
            aria-label={t('editor.underline', 'Underline')}
          />
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<StrikethroughIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('strikethrough')}
            tooltipPos="top"
            aria-label={t('editor.strikethrough', 'Strikethrough')}
          />
          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<CodeIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('code')}
            tooltipPos="top"
            aria-label={t('editor.code', 'Code')}
          />

          <div className="toolbar-divider" />

          <IconButton
            tier="ghost"
            size="sm"
            className="toolbar-btn"
            icon={<ReturnIcon />}
            onMouseDown={handleMouseDown}
            onClick={() => handleAction('insertLineBreak')}
            tooltipPos="top"
            aria-label={t('editor.split_segment', 'Split segment')}
          />
        </div>
      )}
    </>
  );
}
