import React, { useCallback, useEffect, useRef } from 'react';
import { LexicalComposer, type InitialConfigType } from '@lexical/react/LexicalComposer';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin';
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { $generateHtmlFromNodes, $generateNodesFromDOM } from '@lexical/html';
import {
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $splitNode,
  $isTextNode,
  type EditorState,
  type LexicalEditor,
} from 'lexical';
import { setActiveEditor } from '../../stores/transcriptRuntimeStore';
import { convertOldFormatToLexical } from '../../utils/dataMigrationUtils';

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

  // Detect and convert old format (<b>/<i>/<u>) to Lexical format
  const lexicalHtml = convertOldFormatToLexical(initialHtml);

  const initialConfig: InitialConfigType = {
    namespace: `Segment-${segmentId}`,
    editorState: (editor: LexicalEditor) => {
      editorRef.current = editor;
      setActiveEditor(editor);
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
      // eslint-disable-next-line no-console
      console.error('Lexical error:', error);
    },
  };

  const handleEditorReady = useCallback(
    (_editorState: EditorState, editor: LexicalEditor) => {
      editorRef.current = editor;
      setActiveEditor(editor);
    },
    [],
  );

  // Cleanup: clear the active ref when this editor unmounts
  useEffect(() => {
    return () => {
      setActiveEditor(null);
    };
  }, []);

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
          handleSplit(editor, onSplit);
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

  const handleBlur = useCallback(() => {
    onSave(getHtml());
  }, [onSave, getHtml]);

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
    </LexicalComposer>
  );
}

/**
 * Splits the current paragraph at the caret using Lexical's $splitNode API,
 * serializes both halves, and calls onSplit.
 */
function handleSplit(
  editor: LexicalEditor,
  onSplit: (leftHtml: string, rightHtml: string) => void,
): void {
  let leftHtml = '';
  let rightHtml = '';

  editor.update(() => {
    const selection = $getSelection();
    if (!$isRangeSelection(selection) || !selection.isCollapsed()) return;

    const anchorNode = selection.anchor.getNode();
    const parentBlock = anchorNode.getTopLevelElementOrThrow();
    const anchorOffset = selection.anchor.offset;

    // Split the text node at the caret to create two adjacent nodes
    if ($isTextNode(anchorNode)) {
      anchorNode.splitText(anchorOffset);
    }

    // Get all children of the parent block
    const blockChildren = parentBlock.getChildren();
    // Find the split point: it's the index of the right sibling
    const rightSibling = anchorNode.getNextSibling();
    const splitIndex = rightSibling
      ? blockChildren.indexOf(rightSibling)
      : blockChildren.length;

    if (splitIndex <= 0) return;

    // Use $splitNode to split the block at the child index
    const [leftBlock, rightBlock] = $splitNode(parentBlock, splitIndex);

    if (!leftBlock) return;

    const root = parentBlock.getParentOrThrow();

    // Serialize left half: keep only leftBlock in tree
    rightBlock.remove();
    leftHtml = $generateHtmlFromNodes(editor, null);

    // Serialize right half: swap to rightBlock
    leftBlock.remove();
    root.append(rightBlock);
    rightHtml = $generateHtmlFromNodes(editor, null);

    // Restore leftBlock as the only child (current segment)
    root.append(leftBlock);
    rightBlock.remove();
  });

  if (leftHtml && rightHtml) {
    onSplit(leftHtml, rightHtml);
  }
}
