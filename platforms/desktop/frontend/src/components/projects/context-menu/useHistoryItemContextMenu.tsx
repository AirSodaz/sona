import {
  Copy,
  FileText,
  FolderOpen,
  FolderSearch,
  Inbox,
  ListChecks,
  Pencil,
  RotateCcw,
  Tags,
  Trash2,
  X,
} from 'lucide-react';
import React from 'react';
import type { HistoryItem } from '../../../types/history';
import type { ContextMenuOpenRequest } from '../../context-menu/trigger';
import type { ContextMenuAction, OpenContextMenuOptions } from '../../context-menu/types';
import type { TranslationFn } from '../types';

export interface UseHistoryItemContextMenuParams {
  getItemById: (id: string) => HistoryItem | undefined;
  isAllSelected: boolean;
  isLockedLiveDraft: (id: string) => boolean;
  isOpenDisabled: (id: string) => boolean;

  isTrashScope: boolean;
  onAssignProject: (ids: string[]) => void;
  onClearSelection: () => void;
  onCopyTitle?: (item: HistoryItem) => void;
  onCopyTranscript?: (item: HistoryItem) => void;
  onCopyTranscripts?: (items: HistoryItem[]) => void;
  onDeleteHistoryItem: (id: string) => void;
  onDeleteHistoryItems: (ids: string[]) => void;
  onMoveToInbox: (ids: string[]) => void;
  onOpenItem: (item: HistoryItem) => void;
  onRenameHistoryItem: (id: string) => void;
  onRestoreHistoryItems: (ids: string[]) => void;
  onSelectAllVisible: () => void;
  onShowInFolder?: (item: HistoryItem) => void;

  openContextMenu: (options: OpenContextMenuOptions) => void;
  selectedIds: string[];
  t: TranslationFn;
  onMenuOpened?: (contextId: string) => void;
}

export interface HistoryItemContextMenuController {
  openHistoryContextMenu: (id: string, request: ContextMenuOpenRequest) => void;
}

export function useHistoryItemContextMenu({
  getItemById,
  isAllSelected,
  isLockedLiveDraft,
  isOpenDisabled,

  isTrashScope,
  onAssignProject,
  onClearSelection,
  onCopyTitle,
  onCopyTranscript,
  onCopyTranscripts,
  onDeleteHistoryItem,
  onDeleteHistoryItems,
  onMoveToInbox,
  onOpenItem,
  onRenameHistoryItem,
  onRestoreHistoryItems,
  onSelectAllVisible,
  onShowInFolder,

  openContextMenu,
  selectedIds,
  t,
  onMenuOpened,
}: UseHistoryItemContextMenuParams): HistoryItemContextMenuController {
  const openHistoryContextMenu = React.useCallback(
    (id: string, request: ContextMenuOpenRequest) => {
      // Check if this click should trigger a multi-item batch menu
      const isTargetSelected = selectedIds.includes(id);
      const isMultiSelectBatch = selectedIds.length > 1 && isTargetSelected;

      if (isMultiSelectBatch) {
        const selectedItems = selectedIds
          .map((selectedId) => getItemById(selectedId))
          .filter((item): item is HistoryItem => item != null);

        const hasLockedItem = selectedIds.some((selectedId) => isLockedLiveDraft(selectedId));
        const contextId = `workspace:history:batch:${selectedIds.length}`;
        onMenuOpened?.(contextId);

        // Trash multi-selection
        if (isTrashScope) {
          const actions: ContextMenuAction[] = [
            {
              id: 'restore_selected',
              label: t('history.restore_selected', {
                count: selectedIds.length,
                defaultValue: `恢复选中的 ${selectedIds.length} 项`,
              }),
              icon: <RotateCcw size={16} />,
              disabled: hasLockedItem,
              onSelect: () => {
                onRestoreHistoryItems(selectedIds);
              },
            },
            {
              id: 'clear_selection',
              label: t('common.clear_selection', { defaultValue: '取消选择' }),
              icon: <X size={16} />,
              dividerBefore: true,
              onSelect: () => {
                onClearSelection();
              },
            },
            {
              id: 'purge_selected',
              label: t('history.purge_selected', {
                count: selectedIds.length,
                defaultValue: `彻底删除选中的 ${selectedIds.length} 项`,
              }),
              icon: <Trash2 size={16} />,
              tone: 'danger',
              dividerBefore: true,
              shortcut: 'Del',
              disabled: hasLockedItem,
              onSelect: () => {
                onDeleteHistoryItems(selectedIds);
              },
            },
          ];

          openContextMenu({
            contextId,
            ariaLabel: t('history.batch_actions', { defaultValue: '批量操作' }),
            actions,
            ...request,
          });
          return;
        }

        // Normal scope multi-selection
        const actions: ContextMenuAction[] = [
          {
            id: 'assign_project_batch',
            label: t('projects.assign_project_count', {
              count: selectedIds.length,
              defaultValue: `分配项目 (${selectedIds.length})`,
            }),
            icon: <Tags size={16} />,
            disabled: hasLockedItem,
            onSelect: () => {
              onAssignProject(selectedIds);
            },
          },
          {
            id: 'move_to_inbox_batch',
            label: t('projects.move_to_inbox_count', {
              count: selectedIds.length,
              defaultValue: `移至收件箱 (${selectedIds.length})`,
            }),
            icon: <Inbox size={16} />,
            disabled: hasLockedItem,
            onSelect: () => {
              onMoveToInbox(selectedIds);
            },
          },
        ];

        if (onCopyTranscripts && selectedItems.length > 0) {
          actions.push({
            id: 'copy_transcripts_batch',
            label: t('history.copy_transcripts', {
              count: selectedItems.length,
              defaultValue: `批量复制转录内容 (${selectedItems.length})`,
            }),
            icon: <Copy size={16} />,
            dividerBefore: true,
            onSelect: () => {
              onCopyTranscripts(selectedItems);
            },
          });
        }

        actions.push(
          {
            id: 'select_all_visible',
            label: isAllSelected
              ? t('common.clear_selection', { defaultValue: '取消选择' })
              : t('common.select_all', { defaultValue: '全选' }),
            icon: isAllSelected ? <X size={16} /> : <ListChecks size={16} />,
            dividerBefore: true,
            onSelect: () => {
              if (isAllSelected) {
                onClearSelection();
              } else {
                onSelectAllVisible();
              }
            },
          },
          {
            id: 'delete_selected',
            label: t('history.delete_selected', {
              count: selectedIds.length,
              defaultValue: `删除选中的 ${selectedIds.length} 项`,
            }),
            icon: <Trash2 size={16} />,
            tone: 'danger',
            dividerBefore: true,
            shortcut: 'Del',
            disabled: hasLockedItem,
            onSelect: () => {
              onDeleteHistoryItems(selectedIds);
            },
          }
        );

        openContextMenu({
          contextId,
          ariaLabel: t('history.batch_actions', { defaultValue: '批量操作' }),
          actions,
          ...request,
        });
        return;
      }

      // If we right-clicked an item that was NOT in the multi-selection,
      // clear the prior multi-selection so context is unambiguous.
      if (selectedIds.length > 0 && !isTargetSelected) {
        onClearSelection();
      }

      // Single item context menu
      const item = getItemById(id);
      if (!item) {
        return;
      }

      const locked = isLockedLiveDraft(id);
      const contextId = `workspace:history:${id}`;
      onMenuOpened?.(contextId);

      // Trash single item
      if (isTrashScope || item.deletedAt != null) {
        const actions: ContextMenuAction[] = [
          {
            id: 'restore',
            label: t('history.restore', { defaultValue: 'Restore' }),
            icon: <RotateCcw size={16} />,
            onSelect: () => {
              onRestoreHistoryItems([id]);
            },
          },
          {
            id: 'purge',
            label: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
            icon: <Trash2 size={16} />,
            tone: 'danger',
            dividerBefore: true,
            shortcut: 'Del',
            onSelect: () => {
              onDeleteHistoryItem(id);
            },
          },
        ];

        openContextMenu({
          contextId,
          ariaLabel: t('common.actions_for', {
            item: item.title,
            defaultValue: 'Actions for {{item}}',
          }),
          actions,
          ...request,
        });
        return;
      }

      // Normal single item
      const actions: ContextMenuAction[] = [
        {
          id: 'open',
          label: t('common.open', { defaultValue: 'Open' }),
          icon: <FolderOpen size={16} />,
          shortcut: 'Enter',
          disabled: isOpenDisabled(id),
          onSelect: () => {
            onOpenItem(item);
          },
        },
      ];

      // Copy actions
      if (onCopyTranscript) {
        actions.push({
          id: 'copy_transcript',
          label: t('history.copy_transcript', { defaultValue: '复制转录文本' }),
          icon: <Copy size={16} />,
          dividerBefore: true,
          onSelect: () => {
            onCopyTranscript(item);
          },
        });
      }

      if (onCopyTitle) {
        actions.push({
          id: 'copy_title',
          label: t('history.copy_title', { defaultValue: '复制标题' }),
          icon: <FileText size={16} />,
          onSelect: () => {
            onCopyTitle(item);
          },
        });
      }

      // Assignment & Organization
      actions.push({
        id: 'tags',
        label: t('projects.assign_project', { defaultValue: 'Assign Project' }),
        icon: <Tags size={16} />,
        shortcut: 'P',
        dividerBefore: true,
        disabled: locked,
        onSelect: () => {
          onAssignProject([id]);
        },
      });

      if (item.projectId != null) {
        actions.push({
          id: 'move_to_inbox',
          label: t('projects.move_to_inbox', { defaultValue: '移入收件箱' }),
          icon: <Inbox size={16} />,
          disabled: locked,
          onSelect: () => {
            onMoveToInbox([id]);
          },
        });
      }

      actions.push({
        id: 'rename',
        label: t('common.rename', { defaultValue: 'Rename' }),
        icon: <Pencil size={16} />,
        shortcut: 'F2',
        disabled: locked,
        onSelect: () => {
          onRenameHistoryItem(id);
        },
      });

      // File Manager / External Reveal
      if (onShowInFolder) {
        actions.push({
          id: 'reveal_file',
          label: t('history.reveal_in_folder', { defaultValue: '在文件管理器中显示' }),
          icon: <FolderSearch size={16} />,
          dividerBefore: true,
          onSelect: () => {
            onShowInFolder(item);
          },
        });
      }

      // Danger: Delete
      actions.push({
        id: 'delete',
        label: t('common.delete', { defaultValue: 'Delete' }),
        icon: <Trash2 size={16} />,
        tone: 'danger',
        dividerBefore: true,
        shortcut: 'Del',
        disabled: locked,
        onSelect: () => {
          onDeleteHistoryItem(id);
        },
      });

      openContextMenu({
        contextId,
        ariaLabel: t('common.actions_for', {
          item: item.title,
          defaultValue: 'Actions for {{item}}',
        }),
        actions,
        ...request,
      });
    },
    [
      getItemById,
      isAllSelected,
      isLockedLiveDraft,
      isOpenDisabled,

      isTrashScope,
      onAssignProject,
      onClearSelection,
      onCopyTitle,
      onCopyTranscript,
      onCopyTranscripts,
      onDeleteHistoryItem,
      onDeleteHistoryItems,
      onMenuOpened,
      onMoveToInbox,
      onOpenItem,
      onRenameHistoryItem,
      onRestoreHistoryItems,
      onSelectAllVisible,
      onShowInFolder,
      openContextMenu,
      selectedIds,
      t,
    ]
  );

  return { openHistoryContextMenu };
}
