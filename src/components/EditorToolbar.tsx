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

export function EditorToolbar(): React.JSX.Element | null {
    const { t } = useTranslation();
    const editingSegmentId = useTranscriptStore((state) => state.editingSegmentId);
    const [isVisible, setIsVisible] = useState(false);

    useEffect(() => {
        if (editingSegmentId) {
            setIsVisible(true);
        } else {
            // Delay hiding slightly to allow for blur events to complete?
            // Or just hide immediately. The user said "Hide it automatically when editing stops."
            setIsVisible(false);
        }
    }, [editingSegmentId]);

    if (!isVisible) return null;

    const handleAction = (command: string, value?: string) => {
        document.execCommand(command, false, value);
    };

    // Prevent button click from stealing focus from the editable element
    const handleMouseDown = (e: React.MouseEvent) => {
        e.preventDefault();
    };

    return (
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
                onClick={() => handleAction('insertLineBreak')} // insertLineBreak inserts a <br> usually
                data-tooltip={t('editor.line_break', 'Line break')}
                data-tooltip-pos="top"
                aria-label={t('editor.line_break', 'Line break')}
            >
                <ReturnIcon />
            </button>
        </div>
    );
}
