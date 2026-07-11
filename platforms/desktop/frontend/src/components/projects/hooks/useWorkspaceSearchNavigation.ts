import React, { useCallback, useEffect } from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import { useDialogStore } from '../../../stores/dialogStore';
import { useErrorDialogStore } from '../../../stores/errorDialogStore';
import { getWorkspaceSearchResultDomId } from '../../../utils/workspaceSearch';

interface UseWorkspaceSearchNavigationParams {
  activeSearchResultId: string | null;
  filteredItems: HistoryItemType[];
  isSelectionMode: boolean;
  onOpenItem: (item: HistoryItemType) => void | Promise<void>;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  searchQuery: string;
  setActiveSearchResultId: (nextValue: React.SetStateAction<string | null>) => void;
  setSearchQuery: (value: string) => void;
}

export function useWorkspaceSearchNavigation({
  activeSearchResultId,
  filteredItems,
  isSelectionMode,
  onOpenItem,
  searchInputRef,
  searchQuery,
  setActiveSearchResultId,
  setSearchQuery,
}: UseWorkspaceSearchNavigationParams) {
  useEffect(() => {
    if (!activeSearchResultId) {
      return;
    }

    const activeElement = document.getElementById(getWorkspaceSearchResultDomId(activeSearchResultId));
    activeElement?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [activeSearchResultId]);

  const focusWorkspaceSearchInput = useCallback(() => {
    if (!searchInputRef.current) {
      return false;
    }

    searchInputRef.current.focus();
    searchInputRef.current.select();
    return true;
  }, [searchInputRef]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'f') {
        return;
      }

      const activeElement = document.activeElement;
      if (activeElement instanceof HTMLElement && activeElement.closest('.projects-detail-pane')) {
        return;
      }

      const isSettingsOpen = !!document.querySelector('.settings-overlay');
      const isDialogOpen = useDialogStore.getState().isOpen;
      const isErrorDialogOpen = useErrorDialogStore.getState().isOpen;
      if (isSettingsOpen || isDialogOpen || isErrorDialogOpen) {
        return;
      }

      if (!focusWorkspaceSearchInput()) {
        return;
      }

      event.preventDefault();
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [focusWorkspaceSearchInput]);

  const moveActiveSearchResult = useCallback((direction: 'next' | 'prev') => {
    if (filteredItems.length === 0) {
      return;
    }

    setActiveSearchResultId((current) => {
      const currentIndex = current
        ? filteredItems.findIndex((item) => item.id === current)
        : -1;
      const fallbackIndex = direction === 'next' ? 0 : filteredItems.length - 1;
      const nextIndex = currentIndex === -1
        ? fallbackIndex
        : (currentIndex + (direction === 'next' ? 1 : -1) + filteredItems.length) % filteredItems.length;

      return filteredItems[nextIndex]?.id ?? null;
    });
  }, [filteredItems, setActiveSearchResultId]);

  return useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();

      if (searchQuery.trim()) {
        setSearchQuery('');
        setActiveSearchResultId(null);
        return;
      }

      setActiveSearchResultId(null);
      searchInputRef.current?.blur();
      return;
    }

    if (isSelectionMode) {
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      moveActiveSearchResult('next');
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      moveActiveSearchResult('prev');
      return;
    }

    if (event.key === 'Enter' && activeSearchResultId) {
      const activeItem = filteredItems.find((item) => item.id === activeSearchResultId);
      if (!activeItem) {
        return;
      }

      event.preventDefault();
      setActiveSearchResultId(null);
      void onOpenItem(activeItem);
    }
  }, [
    activeSearchResultId,
    filteredItems,
    isSelectionMode,
    moveActiveSearchResult,
    onOpenItem,
    searchInputRef,
    searchQuery,
    setActiveSearchResultId,
    setSearchQuery,
  ]);
}
