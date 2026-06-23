import { $generateHtmlFromNodes } from '@lexical/html';
import {
  $getSelection,
  $isRangeSelection,
  $isTextNode,
  $splitNode,
  type LexicalEditor,
} from 'lexical';

/**
 * Splits a paragraph block at the current collapsed caret position and
 * serializes both halves to HTML.
 *
 * After splitting, the editor tree is restored with only the left block,
 * preserving the current segment's content. The caller is responsible for
 * inserting the right block as a new segment.
 *
 * Returns `null` when the selection is invalid or the caret is at a boundary
 * where a split would produce an empty left half.
 */
export function serializeSplitBlocks(
  editor: LexicalEditor,
): { leftHtml: string; rightHtml: string } | null {
  let leftHtml = '';
  let rightHtml = '';

  editor.update(() => {
    const selection = $getSelection();
    if (!$isRangeSelection(selection) || !selection.isCollapsed()) return;

    const anchorNode = selection.anchor.getNode();
    const parentBlock = anchorNode.getTopLevelElementOrThrow();
    const anchorOffset = selection.anchor.offset;

    if ($isTextNode(anchorNode)) {
      anchorNode.splitText(anchorOffset);
    }

    const blockChildren = parentBlock.getChildren();
    const rightSibling = anchorNode.getNextSibling();
    const splitIndex = rightSibling
      ? blockChildren.indexOf(rightSibling)
      : blockChildren.length;

    if (splitIndex <= 0) return;

    const [leftBlock, rightBlock] = $splitNode(parentBlock, splitIndex);
    if (!leftBlock) return;

    const root = leftBlock.getParentOrThrow();

    rightBlock.remove();
    leftHtml = $generateHtmlFromNodes(editor, null);

    leftBlock.remove();
    root.append(rightBlock);
    rightHtml = $generateHtmlFromNodes(editor, null);

    root.clear();
    root.append(leftBlock);
  });

  if (!leftHtml || !rightHtml) return null;
  return { leftHtml, rightHtml };
}
