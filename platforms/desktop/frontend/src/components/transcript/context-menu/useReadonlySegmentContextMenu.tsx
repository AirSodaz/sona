import React, { useCallback, useEffect, useRef } from 'react';
import { Copy, TextSelect } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useContextMenu } from '../../context-menu/useContextMenu';
import {
  createKeyboardContextMenuRequest,
  createPointerContextMenuRequest,
  isContextMenuKeyboardEvent,
  type ContextMenuOpenRequest,
} from '../../context-menu/trigger';
import { logger } from '../../../utils/logger';
import { getEditorShortcut } from './shortcutLabels';

interface UseReadonlySegmentContextMenuOptions {
  segmentId: string;
  rootRef: React.RefObject<HTMLElement | null>;
}

interface ReadonlySegmentContextMenuHandlers {
  onContextMenu: React.MouseEventHandler<HTMLElement>;
  onKeyDown: React.KeyboardEventHandler<HTMLElement>;
}

function getContainedSelectionText(root: HTMLElement): string | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
    return null;
  }

  const range = selection.getRangeAt(0);
  if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) {
    return null;
  }

  const selectedText = selection.toString();
  return selectedText.length > 0 ? selectedText : null;
}

export function useReadonlySegmentContextMenu({
  segmentId,
  rootRef,
}: UseReadonlySegmentContextMenuOptions): ReadonlySegmentContextMenuHandlers {
  const { t } = useTranslation();
  const { closeContextMenu, openContextMenu } = useContextMenu();
  const contextId = `editor:readonly:${segmentId}`;
  const ownsMenuRef = useRef(false);

  useEffect(() => () => {
    if (ownsMenuRef.current) {
      closeContextMenu();
    }
  }, [closeContextMenu, contextId]);

  const openMenu = useCallback((request: ContextMenuOpenRequest) => {
    const root = rootRef.current;
    if (!root) {
      return;
    }

    const copyText = getContainedSelectionText(root) ?? root.textContent ?? '';
    const clipboard = navigator.clipboard;
    const writeText = typeof clipboard?.writeText === 'function'
      ? clipboard.writeText.bind(clipboard)
      : null;

    openContextMenu({
      contextId,
      ariaLabel: t('editor.context_menu_label', { defaultValue: 'Text editing actions' }),
      actions: [
        {
          id: 'copy',
          label: t('common.copy', { defaultValue: 'Copy' }),
          icon: <Copy size={16} />,
          shortcut: getEditorShortcut('copy'),
          disabled: writeText === null,
          onSelect: () => {
            if (!writeText) {
              return;
            }

            void writeText(copyText).catch((error) => {
              void logger.error('[ReadonlySegmentContextMenu] Failed to copy text:', error);
            });
          },
        },
        {
          id: 'select-all',
          label: t('common.select_all', { defaultValue: 'Select All' }),
          icon: <TextSelect size={16} />,
          shortcut: getEditorShortcut('selectAll'),
          onSelect: () => {
            const currentRoot = rootRef.current;
            const selection = window.getSelection();
            if (!currentRoot || !selection) {
              return;
            }

            const range = document.createRange();
            range.selectNodeContents(currentRoot);
            selection.removeAllRanges();
            selection.addRange(range);
          },
        },
      ],
      ...request,
      onClose: () => {
        ownsMenuRef.current = false;
      },
    });
    ownsMenuRef.current = true;
  }, [contextId, openContextMenu, rootRef, t]);

  const onContextMenu = useCallback<React.MouseEventHandler<HTMLElement>>((event) => {
    event.preventDefault();
    event.stopPropagation();
    openMenu(createPointerContextMenuRequest(event));
  }, [openMenu]);

  const onKeyDown = useCallback<React.KeyboardEventHandler<HTMLElement>>((event) => {
    if (!isContextMenuKeyboardEvent(event)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    const anchor = event.target instanceof HTMLElement ? event.target : event.currentTarget;
    openMenu(createKeyboardContextMenuRequest(anchor));
  }, [openMenu]);

  return { onContextMenu, onKeyDown };
}
