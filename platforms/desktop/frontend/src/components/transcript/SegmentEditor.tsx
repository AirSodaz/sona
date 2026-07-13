import React, { useCallback, useEffect, useRef } from 'react';
import { LexicalComposer, type InitialConfigType } from '@lexical/react/LexicalComposer';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin';
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { $generateHtmlFromNodes, $generateNodesFromDOM } from '@lexical/html';
import {
  $getRoot,
  type EditorState,
  type LexicalEditor,
} from 'lexical';
import { setActiveEditor } from '../../stores/transcriptRuntimeStore';
import { convertOldFormatToLexical } from '../../utils/dataMigrationUtils';
import { serializeSplitBlocks } from '../../utils/lexicalSplitUtils';
import { logger } from '../../utils/logger';
import { SegmentEditorContextMenuPlugin } from './context-menu/SegmentEditorContextMenuPlugin';

interface ActiveEditorPluginProps {
  onReady: (editor: LexicalEditor) => void;
}

function ActiveEditorPlugin({ onReady }: ActiveEditorPluginProps): null {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    onReady(editor);
    return () => {
      setActiveEditor(null);
    };
  }, [editor, onReady]);

  return null;
}

/** Props for SegmentEditor. */
export interface SegmentEditorProps {
  segmentId: string;
  /** Initial segment.text — can be old format (<b>/<i>/<u>) or Lexical HTML. */
  initialHtml: string;
  /** Called on Enter or blur with the serialized HTML. */
  onSave: (html: string) => void;
  /** Called on Escape — restore original text. */
  onCancel: () => void;
  /** Called on Shift+Enter — split at caret. */
  onSplit: (leftHtml: string, rightHtml: string) => void;
}

/**
 * Lexical-based rich text editor for a single transcript segment.
 * Replaces the deprecated ContentEditable + execCommand approach.
 * Supports bold, italic, underline formatting with built-in undo/redo.
 */
export function SegmentEditor({
  segmentId,
  initialHtml,
  onSave,
  onCancel,
  onSplit,
}: SegmentEditorProps): React.JSX.Element {
  const editorRef = useRef<LexicalEditor | null>(null);
  const isContextMenuOpenRef = useRef(false);

  // Detect and convert old format (<b>/<i>/<u>) to Lexical format
  const lexicalHtml = convertOldFormatToLexical(initialHtml);

  const initialConfig: InitialConfigType = {
    namespace: `Segment-${segmentId}`,
    editorState: (editor: LexicalEditor) => {
      editorRef.current = editor;
      const dom = new DOMParser().parseFromString(lexicalHtml, 'text/html');
      const nodes = $generateNodesFromDOM(editor, dom);
      const root = $getRoot();
      root.clear();
      for (const node of nodes) {
        root.append(node);
      }
    },
    theme: {
      text: {
        bold: 'editor-bold',
        italic: 'editor-italic',
        underline: 'editor-underline',
      },
    },
    onError: (error: Error) => {
      logger.error('[SegmentEditor] Lexical error:', error);
    },
  };

  const handleActiveEditor = useCallback((editor: LexicalEditor) => {
    editorRef.current = editor;
    setActiveEditor(editor);
  }, []);

  const handleEditorReady = useCallback(
    (_editorState: EditorState, editor: LexicalEditor) => handleActiveEditor(editor),
    [handleActiveEditor],
  );

  /** Serializes current editor state to HTML. */
  const getHtml = useCallback((): string => {
    const editor = editorRef.current;
    if (!editor) return lexicalHtml;
    let html = lexicalHtml;
    editor.read(() => {
      html = $generateHtmlFromNodes(editor);
    });
    return html;
  }, [lexicalHtml]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        e.stopPropagation();
        onSave(getHtml());
        return;
      }

      if (e.key === 'Enter' && e.shiftKey) {
        e.preventDefault();
        e.stopPropagation();
        const editor = editorRef.current;
        if (editor) {
          const result = serializeSplitBlocks(editor);
          if (result) {
            onSplit(result.leftHtml, result.rightHtml);
          }
        }
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onCancel();
        return;
      }
      // Ctrl+B/I/U handled automatically by Lexical RichTextPlugin
    },
    [onSave, onCancel, onSplit, getHtml],
  );

  const handleCommit = useCallback(() => {
    onSave(getHtml());
  }, [onSave, getHtml]);

  const handleBlur = useCallback(() => {
    if (!isContextMenuOpenRef.current) {
      handleCommit();
    }
  }, [handleCommit]);

  const handleContextMenuOpenChange = useCallback((open: boolean) => {
    isContextMenuOpenRef.current = open;
  }, []);

  return (
    <LexicalComposer initialConfig={initialConfig}>
      <RichTextPlugin
        contentEditable={
          <ContentEditable
            className="segment-input"
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            style={{
              whiteSpace: 'pre-wrap',
              overflowY: 'auto',
              minHeight: '1.8em',
              outline: 'none',
            }}
          />
        }
        placeholder={null}
        ErrorBoundary={LexicalErrorBoundary}
      />
      <HistoryPlugin />
      <OnChangePlugin onChange={handleEditorReady} />
      <ActiveEditorPlugin onReady={handleActiveEditor} />
      <SegmentEditorContextMenuPlugin
        segmentId={segmentId}
        onCommit={handleCommit}
        onMenuOpenChange={handleContextMenuOpenChange}
      />
    </LexicalComposer>
  );
}
