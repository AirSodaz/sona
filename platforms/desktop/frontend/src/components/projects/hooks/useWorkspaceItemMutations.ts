import type { TFunction } from 'i18next';
import { historyService } from '../../../services/historyService';
import { historyQueryWorkspace } from '../../../services/tauri/history';
import { transcriptAutoSaveRuntime } from '../../../services/transcriptAutoSaveRuntime';
import type { DialogCheckboxConfig, DialogOptions } from '../../../stores/dialogStore';
import { useHistoryStore } from '../../../stores/historyStore';
import { useProjectStore } from '../../../stores/projectStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../../../stores/transcriptSessionStore';
import { useTranscriptStore } from '../../../stores/transcriptStore';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import { isLiveRecordDraftHistoryItem } from '../../../types/history';

export interface LiveDraftLockState {
  isLocked: boolean;
  isRecording: boolean;
  sourceHistoryId: string | null;
}

export function getLiveDraftLockState(
  isRecording = typeof useTranscriptRuntimeStore?.getState === 'function'
    ? useTranscriptRuntimeStore.getState().isRecording
    : false,
  sourceHistoryId = typeof useTranscriptSessionStore?.getState === 'function'
    ? useTranscriptSessionStore.getState().sourceHistoryId
    : null,
  historyItems = typeof useHistoryStore?.getState === 'function'
    ? useHistoryStore.getState().items
    : [],
  recordingSessionId = typeof useTranscriptStore?.getState === 'function'
    ? useTranscriptStore.getState().recordingSessionId
    : null
): LiveDraftLockState {
  const activeDraftHistoryId = recordingSessionId ?? sourceHistoryId;
  const sourceItem = activeDraftHistoryId
    ? historyItems.find((item) => item.id === activeDraftHistoryId)
    : null;

  const isLocked = Boolean(
    isRecording && (recordingSessionId || (sourceItem && isLiveRecordDraftHistoryItem(sourceItem)))
  );

  return {
    isLocked,
    isRecording,
    sourceHistoryId: activeDraftHistoryId,
  };
}

export interface UseWorkspaceItemMutationsParams {
  isTrashScope: boolean;
  sourceHistoryId: string | null;
  selectedIds: string[];
  trashCount?: number;
  filteredAndSortedItems?: HistoryItemType[];
  clearOpenedItem: () => void;
  clearSelection: () => void;
  refreshHistory: () => Promise<void>;
  confirm: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<boolean>;
  checkboxConfirm: (
    message: string,
    options: Omit<DialogOptions, 'message' | 'type'> & { checkbox: DialogCheckboxConfig }
  ) => Promise<{ confirmed: boolean; checked: boolean }>;
  t: TFunction;
}

export function useWorkspaceItemMutations({
  isTrashScope,
  sourceHistoryId,
  selectedIds,
  trashCount = 0,
  filteredAndSortedItems = [],
  clearOpenedItem,
  clearSelection,
  refreshHistory,
  confirm,
  checkboxConfirm,
  t,
}: UseWorkspaceItemMutationsParams) {
  const handleDeleteHistoryItem = async (id: string) => {
    const initialItem =
      useHistoryStore.getState().items.find((item) => item.id === id) ??
      filteredAndSortedItems.find((item) => item.id === id);
    const initialLockState = getLiveDraftLockState();
    if (
      !initialItem ||
      id === useTranscriptStore.getState().recordingSessionId ||
      (initialLockState.isLocked && id === initialLockState.sourceHistoryId)
    ) {
      return;
    }

    const isTrashItem = initialItem.deletedAt != null;
    let isConfirmed = false;
    let isDirectDelete = false;

    if (isTrashItem) {
      isConfirmed = await confirm(
        t('history.purge_confirm', {
          defaultValue: 'Permanently delete this item? This cannot be undone.',
        }),
        {
          title: t('history.purge_title', { defaultValue: 'Delete Permanently' }),
          confirmLabel: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
          variant: 'error',
        }
      );
    } else {
      const result = await checkboxConfirm(
        t('history.trash_confirm', { defaultValue: 'Move this item to Trash?' }),
        {
          title: t('history.trash_title', { defaultValue: 'Move to Trash' }),
          confirmLabel: t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
          variant: 'error',
          checkbox: {
            label: t('history.delete_directly_checkbox', {
              defaultValue: 'Directly delete permanently (bypass Trash)',
            }),
            checkedConfirmLabel: t('history.delete_permanently', {
              defaultValue: 'Delete Permanently',
            }),
            checkedNotice: t('history.delete_directly_warning', {
              defaultValue: 'This will permanently delete the records and cannot be undone.',
            }),
          },
        }
      );
      isConfirmed = result.confirmed;
      isDirectDelete = result.checked;
    }

    if (!isConfirmed) {
      return;
    }

    const latestItem =
      useHistoryStore.getState().items.find((item) => item.id === id) ??
      filteredAndSortedItems.find((item) => item.id === id);
    const latestLockState = getLiveDraftLockState();
    if (
      !latestItem ||
      id === useTranscriptStore.getState().recordingSessionId ||
      (latestLockState.isLocked && id === latestLockState.sourceHistoryId)
    ) {
      return;
    }

    if (isTrashItem || isDirectDelete) {
      transcriptAutoSaveRuntime.cancelPending([id]);
      await historyService.purgeRecordings([id]);
      if (sourceHistoryId === id) {
        clearOpenedItem();
      }
      transcriptAutoSaveRuntime.cancelPending([id]);
      useTranscriptStore.getState().evictSessionsByHistoryIds([id]);
    } else {
      await useHistoryStore.getState().deleteItem(id);
    }
    await useHistoryStore.getState().refresh();
  };

  const handleDeleteHistoryItems = async (ids: string[]) => {
    if (ids.length === 0) {
      return;
    }

    const initialLockState = getLiveDraftLockState();
    const initialRecordingSessionId = useTranscriptStore.getState().recordingSessionId;
    const candidateIds = ids.filter(
      (id) =>
        id !== initialRecordingSessionId &&
        !(initialLockState.isLocked && id === initialLockState.sourceHistoryId)
    );

    if (candidateIds.length === 0) {
      return;
    }
    let isConfirmed = false;
    let isDirectDelete = false;

    if (isTrashScope) {
      isConfirmed = await confirm(
        t('history.purge_bulk_confirm', {
          count: candidateIds.length,
          defaultValue: `Permanently delete ${candidateIds.length} items? This cannot be undone.`,
        }),
        {
          title: t('history.purge_title', { defaultValue: 'Delete Permanently' }),
          confirmLabel: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
          variant: 'error',
        }
      );
    } else {
      const result = await checkboxConfirm(
        t('history.trash_bulk_confirm', {
          count: candidateIds.length,
          defaultValue: `Move ${candidateIds.length} items to Trash?`,
        }),
        {
          title: t('history.trash_title', { defaultValue: 'Move to Trash' }),
          confirmLabel: t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
          variant: 'error',
          checkbox: {
            label: t('history.delete_directly_checkbox', {
              defaultValue: 'Directly delete permanently (bypass Trash)',
            }),
            checkedConfirmLabel: t('history.delete_permanently', {
              defaultValue: 'Delete Permanently',
            }),
            checkedNotice: t('history.delete_directly_warning', {
              defaultValue: 'This will permanently delete the records and cannot be undone.',
            }),
          },
        }
      );
      isConfirmed = result.confirmed;
      isDirectDelete = result.checked;
    }

    if (!isConfirmed) {
      return;
    }
    const latestLockState = getLiveDraftLockState();
    const latestRecordingSessionId = useTranscriptStore.getState().recordingSessionId;
    const targetIds = candidateIds.filter(
      (id) =>
        id !== latestRecordingSessionId &&
        !(latestLockState.isLocked && id === latestLockState.sourceHistoryId)
    );

    if (targetIds.length === 0) {
      return;
    }

    if (isTrashScope || isDirectDelete) {
      transcriptAutoSaveRuntime.cancelPending(targetIds);
      await historyService.purgeRecordings(targetIds);
      if (sourceHistoryId && targetIds.includes(sourceHistoryId)) {
        clearOpenedItem();
      }
      transcriptAutoSaveRuntime.cancelPending(targetIds);
      useTranscriptStore.getState().evictSessionsByHistoryIds(targetIds);
    } else {
      await useHistoryStore.getState().deleteItems(targetIds);
    }
    await refreshHistory();
    clearSelection();
  };

  const handleDeleteSelected = async () => {
    await handleDeleteHistoryItems(selectedIds);
  };

  const handleRestoreHistoryItems = async (ids: string[]) => {
    await historyService.restoreRecordings(ids);
    await refreshHistory();
    clearSelection();
  };

  const handleMoveToInbox = async (ids: string[]) => {
    await useProjectStore.getState().moveItemsToProject(ids, null);
    await refreshHistory();
    clearSelection();
  };

  const handleEmptyTrash = async () => {
    if (trashCount === 0) return;
    const confirmed = await confirm(
      t('history.empty_trash_confirm', {
        count: trashCount,
        defaultValue: `Permanently delete all ${trashCount} items in Trash? This cannot be undone.`,
      }),
      {
        title: t('history.empty_trash', { defaultValue: 'Empty Trash' }),
        confirmLabel: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
        variant: 'error',
      }
    );
    if (!confirmed) return;

    const ids: string[] = [];
    let offset = 0;
    const limit = 200;
    while (true) {
      const page = await historyQueryWorkspace({
        scope: { kind: 'trash' },
        query: '',
        filterType: 'all',
        dateFilter: 'all',
        sortOrder: 'newest',
        limit,
        offset,
      });
      ids.push(...page.filteredItems.map((item) => item.id));
      if (!page.hasMore || page.filteredItems.length === 0) break;
      offset += page.filteredItems.length;
    }
    transcriptAutoSaveRuntime.cancelPending(ids);
    await historyService.purgeRecordings(ids);
    if (sourceHistoryId && ids.includes(sourceHistoryId)) {
      clearOpenedItem();
    }
    transcriptAutoSaveRuntime.cancelPending(ids);
    useTranscriptStore.getState().evictSessionsByHistoryIds(ids);
    await refreshHistory();
  };

  return {
    handleDeleteHistoryItem,
    handleDeleteHistoryItems,
    handleDeleteSelected,
    handleRestoreHistoryItems,
    handleMoveToInbox,
    handleEmptyTrash,
  };
}
