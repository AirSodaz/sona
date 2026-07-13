import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $getSelection,
  $setSelection,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  FORMAT_TEXT_COMMAND,
  REMOVE_TEXT_COMMAND,
  SELECT_ALL_COMMAND,
  type BaseSelection,
  type TextFormatType,
} from 'lexical';
import {
  Bold as BoldIcon,
  ClipboardPaste,
  Copy,
  Italic as ItalicIcon,
  Scissors,
  TextSelect,
  Underline as UnderlineIcon,
} from 'lucide-react';
import { useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { ContextMenuAction, ContextMenuCloseReason } from '../../context-menu/types';
import { useContextMenu } from '../../context-menu/useContextMenu';
import { logger } from '../../../utils/logger';
import { getEditorShortcut } from './shortcutLabels';

interface SegmentEditorContextMenuPluginProps {
  segmentId: string;
  onCommit: () => void;
  onMenuOpenChange: (open: boolean) => void;
}

export function SegmentEditorContextMenuPlugin({
  segmentId,
  onCommit,
  onMenuOpenChange,
}: SegmentEditorContextMenuPluginProps): null {
  const [editor] = useLexicalComposerContext();
  const { t } = useTranslation();
  const { closeContextMenu, openContextMenu } = useContextMenu();
  const contextId = `editor:editing:${segmentId}`;
  const mountedRef = useRef(true);
  const ownsMenuRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (ownsMenuRef.current) {
        closeContextMenu();
      }
    };
  }, [closeContextMenu, contextId]);

  const restoreEditor = useCallback((
    selection: BaseSelection | null,
    action?: () => void,
  ): boolean => {
    const root = editor.getRootElement();
    const selectionSnapshot = selection?.clone() ?? null;
    if (!mountedRef.current || !root?.isConnected) {
      return false;
    }

    window.setTimeout(() => {
      const currentRoot = editor.getRootElement();
      if (!mountedRef.current || !currentRoot?.isConnected) return;
      currentRoot.focus();
      editor.update(() => {
        if (selectionSnapshot) {
          $setSelection(selectionSnapshot);
        }
        action?.();
      }, { discrete: true });
    }, 0);
    return true;
  }, [editor]);

  const openMenu = useCallback((anchor: HTMLElement, point: { x: number; y: number }, invocation: 'pointer' | 'keyboard') => {
    let selectionSnapshot: BaseSelection | null = null;
    let selectedText = '';
    editor.getEditorState().read(() => {
      const selection = $getSelection();
      selectionSnapshot = selection?.clone() ?? null;
      selectedText = selection?.getTextContent() ?? '';
    });

    const clipboard = navigator.clipboard;
    const hasSelection = selectedText.length > 0;
    const canWrite = typeof clipboard?.writeText === 'function';
    const canRead = typeof clipboard?.readText === 'function';

    const copy = () => {
      if (!canWrite) return;
      restoreEditor(selectionSnapshot);
      void clipboard.writeText(selectedText).catch((error) => {
        logger.error('[SegmentEditorContextMenu] Failed to copy text:', error);
      });
    };
    const cut = () => {
      if (!canWrite) return;
      restoreEditor(selectionSnapshot);
      void clipboard.writeText(selectedText).then(() => {
        restoreEditor(selectionSnapshot, () => {
          editor.dispatchCommand(REMOVE_TEXT_COMMAND, null);
        });
      }).catch((error) => {
        logger.error('[SegmentEditorContextMenu] Failed to cut text:', error);
      });
    };
    const paste = () => {
      if (!canRead) return;
      restoreEditor(selectionSnapshot);
      void clipboard.readText().then((text) => {
        restoreEditor(selectionSnapshot, () => {
          editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, text);
        });
      }).catch((error) => {
        logger.error('[SegmentEditorContextMenu] Failed to paste text:', error);
      });
    };
    const selectAll = () => {
      restoreEditor(selectionSnapshot, () => {
        editor.dispatchCommand(SELECT_ALL_COMMAND, new KeyboardEvent('keydown', { key: 'a' }));
      });
    };
    const format = (textFormat: TextFormatType) => {
      restoreEditor(selectionSnapshot, () => {
        editor.dispatchCommand(FORMAT_TEXT_COMMAND, textFormat);
      });
    };

    const actions: ContextMenuAction[] = [
      { id: 'cut', label: t('common.cut'), icon: <Scissors />, shortcut: getEditorShortcut('cut'), disabled: !hasSelection || !canWrite, onSelect: cut },
      { id: 'copy', label: t('common.copy'), icon: <Copy />, shortcut: getEditorShortcut('copy'), disabled: !hasSelection || !canWrite, onSelect: copy },
      { id: 'paste', label: t('common.paste'), icon: <ClipboardPaste />, shortcut: getEditorShortcut('paste'), disabled: !canRead, onSelect: paste },
      { id: 'select-all', label: t('common.select_all'), icon: <TextSelect />, shortcut: getEditorShortcut('selectAll'), dividerBefore: true, onSelect: selectAll },
      { id: 'bold', label: t('editor.bold'), icon: <BoldIcon />, shortcut: getEditorShortcut('bold'), dividerBefore: true, onSelect: () => format('bold') },
      { id: 'italic', label: t('editor.italic'), icon: <ItalicIcon />, shortcut: getEditorShortcut('italic'), onSelect: () => format('italic') },
      { id: 'underline', label: t('editor.underline'), icon: <UnderlineIcon />, shortcut: getEditorShortcut('underline'), onSelect: () => format('underline') },
    ];

    openContextMenu({
      contextId,
      ariaLabel: t('editor.context_menu_label'),
      actions,
      anchor,
      point,
      invocation,
      onClose: (reason: ContextMenuCloseReason) => {
        ownsMenuRef.current = false;
        if (!mountedRef.current) return;
        onMenuOpenChange(false);
        if (reason === 'escape') restoreEditor(selectionSnapshot);
        if (reason !== 'action' && reason !== 'escape') onCommit();
      },
    });
    ownsMenuRef.current = true;
    onMenuOpenChange(true);
  }, [contextId, editor, onCommit, onMenuOpenChange, openContextMenu, restoreEditor, t]);

  useEffect(() => {
    let currentRoot: HTMLElement | null = null;
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      openMenu(event.currentTarget as HTMLElement, { x: event.clientX, y: event.clientY }, 'pointer');
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'ContextMenu' && !(event.key === 'F10' && event.shiftKey)) return;
      event.preventDefault();
      event.stopPropagation();
      const root = event.currentTarget as HTMLElement;
      const rect = root.getBoundingClientRect();
      openMenu(root, { x: rect.left + 12, y: rect.top + 12 }, 'keyboard');
    };

    const unregisterRootListener = editor.registerRootListener((root, previousRoot) => {
      previousRoot?.removeEventListener('contextmenu', handleContextMenu);
      previousRoot?.removeEventListener('keydown', handleKeyDown);
      root?.addEventListener('contextmenu', handleContextMenu);
      root?.addEventListener('keydown', handleKeyDown);
      currentRoot = root;
    });

    return () => {
      currentRoot?.removeEventListener('contextmenu', handleContextMenu);
      currentRoot?.removeEventListener('keydown', handleKeyDown);
      unregisterRootListener();
    };
  }, [editor, openMenu]);

  return null;
}
