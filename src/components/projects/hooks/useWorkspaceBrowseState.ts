import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import type {
  TranslationFn,
  WorkspaceQueryRequest,
} from '../types';
import { buildWorkspaceViewModel } from './workspaceViewModel';
import { useWorkspaceBrowseControls } from './useWorkspaceBrowseControls';
import { useWorkspaceQuery } from './workspaceQuery';
import { useWorkspaceSearchNavigation } from './useWorkspaceSearchNavigation';
import { useEscapeKey } from '../../../hooks/useEscapeKey';

interface UseWorkspaceBrowseStateParams {
  activeProjectId: string | null;
  historyItems: HistoryItemType[];
  projects: ProjectRecord[];
  filterMenuRef: React.RefObject<HTMLDivElement | null>;
  isSelectionMode: boolean;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  t: TranslationFn;
  onOpenItem: (item: HistoryItemType) => void | Promise<void>;
}

export function useWorkspaceBrowseState({
  activeProjectId,
  historyItems,
  projects,
  filterMenuRef,
  isSelectionMode,
  searchInputRef,
  t,
  onOpenItem,
}: UseWorkspaceBrowseStateParams) {
  const [isScrolled, setIsScrolled] = useState(false);
  const controls = useWorkspaceBrowseControls({
    activeProjectId,
    projects,
  });
  const { isFilterMenuOpen, setIsFilterMenuOpen } = controls;

  const handleScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const target = event.currentTarget;
    const scrollTop = target.scrollTop;
    const scrollableHeight = target.scrollHeight - target.clientHeight;

    if (scrollTop > 10 && scrollableHeight > 250) {
      setIsScrolled(true);
    } else if (scrollTop <= 10) {
      setIsScrolled(false);
    }
  }, []);

  useEffect(() => {
    if (!isFilterMenuOpen) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (filterMenuRef.current?.contains(event.target as Node)) {
        return;
      }

      setIsFilterMenuOpen(false);
    };

    document.addEventListener('mousedown', handlePointerDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
    };
  }, [filterMenuRef, isFilterMenuOpen, setIsFilterMenuOpen]);

  useEscapeKey(() => {
    setIsFilterMenuOpen(false);
  }, {
    enabled: isFilterMenuOpen,
    checkTopMost: true,
    containerRef: filterMenuRef,
  });

  const workspaceQueryScope = useMemo<WorkspaceQueryRequest['scope']>(() => {
    if (controls.isAllItemsScope) {
      return { kind: 'all' };
    }

    if (controls.isInboxScope || !controls.browseProjectId) {
      return { kind: 'inbox' };
    }

    return { kind: 'project', projectId: controls.browseProjectId };
  }, [controls.browseProjectId, controls.isAllItemsScope, controls.isInboxScope]);

  const workspaceQueryResult = useWorkspaceQuery({
    scope: workspaceQueryScope,
    searchQuery: controls.searchQuery,
    filterType: controls.filterType,
    dateFilter: controls.dateFilter,
    sortOrder: controls.sortOrder,
    historyItems,
  });

  const scopedItems = workspaceQueryResult.scopedItems;
  const filteredAndSortedItems = workspaceQueryResult.filteredItems;

  const searchMatchByItemId = useMemo(
    () => new Map(Object.entries(workspaceQueryResult.searchMatchByItemId)),
    [workspaceQueryResult.searchMatchByItemId],
  );

  const activeSearchResultId = useMemo(() => {
    if (isSelectionMode || !controls.activeSearchResultIdState) {
      return null;
    }

    return filteredAndSortedItems.some((item) => item.id === controls.activeSearchResultIdState)
      ? controls.activeSearchResultIdState
      : null;
  }, [controls.activeSearchResultIdState, filteredAndSortedItems, isSelectionMode]);

  const viewModel = useMemo(() => buildWorkspaceViewModel({
    browseProject: controls.browseProject,
    browseScope: controls.browseScope,
    dateFilter: controls.dateFilter,
    filterType: controls.filterType,
    projects,
    queryResult: workspaceQueryResult,
    t,
  }), [
    controls.browseProject,
    controls.browseScope,
    controls.dateFilter,
    controls.filterType,
    projects,
    t,
    workspaceQueryResult,
  ]);

  const handleWorkspaceSearchInputKeyDown = useWorkspaceSearchNavigation({
    activeSearchResultId,
    filteredItems: filteredAndSortedItems,
    isSelectionMode,
    onOpenItem,
    searchInputRef,
    searchQuery: controls.searchQuery,
    setActiveSearchResultId: controls.setActiveSearchResultId,
    setSearchQuery: controls.setSearchQuery,
  });

  return {
    browseScope: controls.browseScope,
    setBrowseScope: controls.setBrowseScope,
    isAllItemsScope: controls.isAllItemsScope,
    isInboxScope: controls.isInboxScope,
    browseProjectId: controls.browseProjectId,
    browseProject: controls.browseProject,
    isFilterMenuOpen: controls.isFilterMenuOpen,
    setIsFilterMenuOpen: controls.setIsFilterMenuOpen,
    searchQuery: controls.searchQuery,
    setSearchQuery: controls.setSearchQuery,
    activeSearchResultId,
    setActiveSearchResultId: controls.setActiveSearchResultId,
    filterType: controls.filterType,
    setFilterType: controls.setFilterType,
    dateFilter: controls.dateFilter,
    setDateFilter: controls.setDateFilter,
    sortOrder: controls.sortOrder,
    setSortOrder: controls.setSortOrder,
    isScrolled,
    handleScroll,
    scopedItems,
    filteredAndSortedItems,
    searchMatchByItemId,
    itemCounts: viewModel.itemCounts,
    headerTitle: viewModel.headerTitle,
    headerDescription: viewModel.headerDescription,
    showWorkflowActions: viewModel.showWorkflowActions,
    headerIcon: viewModel.headerIcon,
    searchInputLabel: viewModel.searchInputLabel,
    summaryChips: viewModel.summaryChips,
    moveOptions: viewModel.moveOptions,
    filterTypeOptions: viewModel.filterTypeOptions,
    dateFilterOptions: viewModel.dateFilterOptions,
    sortOptions: viewModel.sortOptions,
    activeFilterCount: viewModel.activeFilterCount,
    hasActiveFilters: viewModel.hasActiveFilters,
    filterPopoverHint: viewModel.filterPopoverHint,
    handleWorkspaceSearchInputKeyDown,
    resetBrowseState: controls.resetBrowseState,
  };
}
