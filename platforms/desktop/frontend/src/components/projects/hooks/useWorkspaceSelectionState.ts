import { useCallback, useRef, useState } from 'react';
import type React from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import { INBOX_SCOPE } from '../constants';

interface UseWorkspaceSelectionStateParams {
  browseProjectId: string | null;
  isAllItemsScope: boolean;
  isSelectionMode: boolean;
  projects: ProjectRecord[];
  setIsSelectionMode: React.Dispatch<React.SetStateAction<boolean>>;
}

export function useWorkspaceSelectionState({
  browseProjectId,
  isAllItemsScope,
  isSelectionMode,
  projects,
  setIsSelectionMode,
}: UseWorkspaceSelectionStateParams) {
  const visibleItemsRef = useRef<HistoryItemType[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const defaultMoveTarget = browseProjectId ? INBOX_SCOPE : projects[0]?.id || INBOX_SCOPE;
  const [moveTarget, setMoveTarget] = useState(defaultMoveTarget);

  const syncVisibleItems = useCallback((items: HistoryItemType[]) => {
    visibleItemsRef.current = items;
    const visibleIds = new Set(items.map((item) => item.id));
    setSelectedIds((current) => {
      const next = current.filter((id) => visibleIds.has(id));
      return next.length === current.length ? current : next;
    });
  }, []);

  const toggleSelection = useCallback((id: string) => {
    setSelectedIds((current) => (
      current.includes(id) ? current.filter((itemId) => itemId !== id) : [...current, id]
    ));
  }, []);

  const clearSelection = useCallback(() => {
    setSelectedIds([]);
    setIsSelectionMode(false);
    setMoveTarget(defaultMoveTarget);
  }, [defaultMoveTarget, setIsSelectionMode]);

  const toggleSelectionMode = useCallback(() => {
    setIsSelectionMode((value) => !value);
    setSelectedIds([]);
    setMoveTarget(defaultMoveTarget);
  }, [defaultMoveTarget, setIsSelectionMode]);

  const handleToggleSelectAll = useCallback(() => {
    const visibleItems = visibleItemsRef.current;
    if (selectedIds.length === visibleItems.length && visibleItems.length > 0) {
      setSelectedIds([]);
      return;
    }

    setSelectedIds(visibleItems.map((item) => item.id));
  }, [selectedIds]);

  const currentScopeMoveTarget = isAllItemsScope ? null : browseProjectId || INBOX_SCOPE;

  return {
    isSelectionMode,
    setIsSelectionMode,
    selectedIds,
    setSelectedIds,
    moveTarget,
    setMoveTarget,
    currentScopeMoveTarget,
    syncVisibleItems,
    toggleSelection,
    clearSelection,
    toggleSelectionMode,
    handleToggleSelectAll,
  };
}
