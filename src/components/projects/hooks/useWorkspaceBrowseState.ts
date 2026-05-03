import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import { historyQueryWorkspace } from '../../../services/tauri/history';
import { useDialogStore } from '../../../stores/dialogStore';
import { useErrorDialogStore } from '../../../stores/errorDialogStore';
import { getWorkspaceSearchResultDomId } from '../../../utils/workspaceSearch';
import {
  ALL_ITEMS_SCOPE,
  DEFAULT_DATE_FILTER,
  DEFAULT_FILTER_TYPE,
  DEFAULT_SORT_ORDER,
  INBOX_SCOPE,
} from '../constants';
import type {
  ProjectBrowseScope,
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
  ProjectSummaryChip,
  TranslationFn,
  WorkspaceQueryRequest,
  WorkspaceQueryResult,
} from '../types';
import {
  formatSummaryDuration,
  formatTimestamp,
  renderScopeIcon,
} from '../utils';

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

const EMPTY_WORKSPACE_QUERY_RESULT: WorkspaceQueryResult = {
  filteredItems: [],
  scopedItems: [],
  scopedItemIds: [],
  searchMatchByItemId: {},
  summary: {
    totalItems: 0,
    totalDuration: 0,
    latestTimestamp: null,
    recordingCount: 0,
    batchCount: 0,
  },
  itemCounts: {
    inbox: 0,
    byProjectId: {},
  },
};

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
  const [browseScopeState, setBrowseScopeState] = useState<ProjectBrowseScope>(() => activeProjectId || INBOX_SCOPE);
  const [isFilterMenuOpen, setIsFilterMenuOpenState] = useState(false);
  const [searchQuery, setSearchQueryState] = useState('');
  const [activeSearchResultIdState, setActiveSearchResultIdState] = useState<string | null>(null);
  const [filterType, setFilterTypeState] = useState<ProjectFilterType>(DEFAULT_FILTER_TYPE);
  const [dateFilter, setDateFilterState] = useState<ProjectDateFilter>(DEFAULT_DATE_FILTER);
  const [sortOrder, setSortOrderState] = useState<ProjectSortOrder>(DEFAULT_SORT_ORDER);
  const [isScrolled, setIsScrolled] = useState(false);

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

  const browseScope = useMemo<ProjectBrowseScope>(() => {
    if (browseScopeState === ALL_ITEMS_SCOPE || browseScopeState === INBOX_SCOPE) {
      return browseScopeState;
    }

    if (projects.some((item) => item.id === browseScopeState)) {
      return browseScopeState;
    }

    return activeProjectId || INBOX_SCOPE;
  }, [activeProjectId, browseScopeState, projects]);

  const isAllItemsScope = browseScope === ALL_ITEMS_SCOPE;
  const isInboxScope = browseScope === INBOX_SCOPE;
  const browseProjectId = !isAllItemsScope && !isInboxScope ? browseScope : null;
  const browseProject = useMemo(
    () => projects.find((item) => item.id === browseProjectId) || null,
    [browseProjectId, projects],
  );

  const setActiveSearchResultId = useCallback((nextValue: React.SetStateAction<string | null>) => {
    setActiveSearchResultIdState((current) => (
      typeof nextValue === 'function'
        ? nextValue(current)
        : nextValue
    ));
  }, []);

  const setSearchQuery = useCallback((value: string) => {
    setSearchQueryState(value);
    setActiveSearchResultIdState(null);
  }, []);

  const setFilterType = useCallback((value: ProjectFilterType) => {
    setFilterTypeState(value);
    setActiveSearchResultIdState(null);
  }, []);

  const setDateFilter = useCallback((value: ProjectDateFilter) => {
    setDateFilterState(value);
    setActiveSearchResultIdState(null);
  }, []);

  const setSortOrder = useCallback((value: ProjectSortOrder) => {
    setSortOrderState(value);
    setActiveSearchResultIdState(null);
  }, []);

  const setIsFilterMenuOpen = useCallback((value: React.SetStateAction<boolean>) => {
    setIsFilterMenuOpenState(value);
  }, []);

  const setBrowseScope = useCallback((value: ProjectBrowseScope) => {
    setBrowseScopeState(value);
    setSearchQueryState('');
    setFilterTypeState(DEFAULT_FILTER_TYPE);
    setDateFilterState(DEFAULT_DATE_FILTER);
    setIsFilterMenuOpenState(false);
    setActiveSearchResultIdState(null);
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

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') {
        return;
      }

      setIsFilterMenuOpen(false);
    };

    document.addEventListener('mousedown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [filterMenuRef, isFilterMenuOpen, setIsFilterMenuOpen]);

  const [workspaceQueryResult, setWorkspaceQueryResult] = useState<WorkspaceQueryResult>(EMPTY_WORKSPACE_QUERY_RESULT);
  const workspaceQueryRequestIdRef = useRef(0);
  const workspaceQueryScope = useMemo<WorkspaceQueryRequest['scope']>(() => {
    if (isAllItemsScope) {
      return { kind: 'all' };
    }

    if (isInboxScope || !browseProjectId) {
      return { kind: 'inbox' };
    }

    return { kind: 'project', projectId: browseProjectId };
  }, [browseProjectId, isAllItemsScope, isInboxScope]);

  useEffect(() => {
    const requestId = workspaceQueryRequestIdRef.current + 1;
    workspaceQueryRequestIdRef.current = requestId;

    void historyQueryWorkspace({
      scope: workspaceQueryScope,
      query: searchQuery,
      filterType,
      dateFilter,
      sortOrder,
    })
      .then((result) => {
        if (workspaceQueryRequestIdRef.current !== requestId) {
          return;
        }

        setWorkspaceQueryResult(result);
      })
      .catch(() => {
        if (workspaceQueryRequestIdRef.current !== requestId) {
          return;
        }

        setWorkspaceQueryResult(EMPTY_WORKSPACE_QUERY_RESULT);
      });
  }, [dateFilter, filterType, historyItems, searchQuery, sortOrder, workspaceQueryScope]);

  const scopedItems = useMemo(
    () => workspaceQueryResult.scopedItems,
    [workspaceQueryResult.scopedItems],
  );

  const filteredAndSortedItems = workspaceQueryResult.filteredItems;

  const searchMatchByItemId = useMemo(
    () => new Map(Object.entries(workspaceQueryResult.searchMatchByItemId)),
    [workspaceQueryResult.searchMatchByItemId],
  );

  const activeSearchResultId = useMemo(() => {
    if (isSelectionMode || !activeSearchResultIdState) {
      return null;
    }

    return filteredAndSortedItems.some((item) => item.id === activeSearchResultIdState)
      ? activeSearchResultIdState
      : null;
  }, [activeSearchResultIdState, filteredAndSortedItems, isSelectionMode]);

  const itemCounts = useMemo(() => {
    const counts = new Map<string | null, number>();
    counts.set(null, workspaceQueryResult.itemCounts.inbox);
    Object.entries(workspaceQueryResult.itemCounts.byProjectId).forEach(([projectId, count]) => {
      counts.set(projectId, count);
    });
    return counts;
  }, [workspaceQueryResult.itemCounts]);

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

  const moveOptions = useMemo(
    () => [
      { value: INBOX_SCOPE, label: t('projects.inbox', { defaultValue: 'Inbox' }) },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t],
  );

  const filterTypeOptions = useMemo(
    () => [
      { value: 'all', label: t('projects.filter_all_types', { defaultValue: 'All types' }) },
      { value: 'recording', label: t('projects.filter_recordings', { defaultValue: 'Recordings' }) },
      { value: 'batch', label: t('projects.filter_batch', { defaultValue: 'Batch imports' }) },
    ],
    [t],
  );

  const dateFilterOptions = useMemo(
    () => [
      { value: 'all', label: t('projects.date_all', { defaultValue: 'Any time' }) },
      { value: 'today', label: t('projects.date_today', { defaultValue: 'Today' }) },
      { value: 'week', label: t('projects.date_week', { defaultValue: 'Last 7 days' }) },
      { value: 'month', label: t('projects.date_month', { defaultValue: 'Last 30 days' }) },
    ],
    [t],
  );

  const sortOptions = useMemo(
    () => [
      { value: 'newest', label: t('projects.sort_newest', { defaultValue: 'Newest first' }) },
      { value: 'oldest', label: t('projects.sort_oldest', { defaultValue: 'Oldest first' }) },
      { value: 'duration_desc', label: t('projects.sort_duration_desc', { defaultValue: 'Longest first' }) },
      { value: 'duration_asc', label: t('projects.sort_duration_asc', { defaultValue: 'Shortest first' }) },
      { value: 'title_asc', label: t('projects.sort_title_asc', { defaultValue: 'Title A-Z' }) },
    ],
    [t],
  );

  const activeFilterLabels = useMemo(() => {
    const labels: string[] = [];

    if (filterType !== DEFAULT_FILTER_TYPE) {
      const typeLabel = filterTypeOptions.find((option) => option.value === filterType)?.label;
      if (typeLabel) {
        labels.push(typeLabel);
      }
    }

    if (dateFilter !== DEFAULT_DATE_FILTER) {
      const dateLabel = dateFilterOptions.find((option) => option.value === dateFilter)?.label;
      if (dateLabel) {
        labels.push(dateLabel);
      }
    }

    return labels;
  }, [dateFilter, dateFilterOptions, filterType, filterTypeOptions]);

  const activeFilterCount = activeFilterLabels.length;
  const hasActiveFilters = activeFilterCount > 0;
  const filterPopoverHint = hasActiveFilters
    ? activeFilterLabels.join(' · ')
    : t('projects.filter_menu_hint', {
      defaultValue: 'Refine the current workspace view by type or time.',
    });

  const moveActiveSearchResult = useCallback((direction: 'next' | 'prev') => {
    if (filteredAndSortedItems.length === 0) {
      return;
    }

    setActiveSearchResultId((current) => {
      const currentIndex = current
        ? filteredAndSortedItems.findIndex((item) => item.id === current)
        : -1;
      const fallbackIndex = direction === 'next' ? 0 : filteredAndSortedItems.length - 1;
      const nextIndex = currentIndex === -1
        ? fallbackIndex
        : (currentIndex + (direction === 'next' ? 1 : -1) + filteredAndSortedItems.length) % filteredAndSortedItems.length;

      return filteredAndSortedItems[nextIndex]?.id ?? null;
    });
  }, [filteredAndSortedItems, setActiveSearchResultId]);

  const handleWorkspaceSearchInputKeyDown = useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
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
      const activeItem = filteredAndSortedItems.find((item) => item.id === activeSearchResultId);
      if (!activeItem) {
        return;
      }

      event.preventDefault();
      setActiveSearchResultId(null);
      void onOpenItem(activeItem);
    }
  }, [
    activeSearchResultId,
    filteredAndSortedItems,
    isSelectionMode,
    moveActiveSearchResult,
    onOpenItem,
    setActiveSearchResultId,
    setSearchQuery,
    searchInputRef,
    searchQuery,
  ]);

  const resetBrowseState = useCallback(() => {
    setSearchQueryState('');
    setFilterTypeState(DEFAULT_FILTER_TYPE);
    setDateFilterState(DEFAULT_DATE_FILTER);
    setActiveSearchResultIdState(null);
  }, []);

  const headerTitle = isAllItemsScope
    ? t('projects.all_items', { defaultValue: 'All Items' })
    : browseProject?.name || t('projects.inbox', { defaultValue: 'Inbox' });
  const headerDescription = isAllItemsScope
    ? t('projects.all_items_description', {
      defaultValue: 'Browse everything across Inbox and your projects.',
    })
    : browseProject
    ? browseProject.description
    : t('projects.inbox_description', {
      defaultValue: 'Inbox collects unassigned recordings and imports.',
    });
  const showWorkflowActions = !isAllItemsScope;
  const headerIcon = renderScopeIcon(browseScope, browseProject);
  const searchInputLabel = isAllItemsScope
    ? t('projects.search_placeholder_all_items', { defaultValue: 'Search All Items...' })
    : browseProject
    ? t('projects.search_placeholder_project', {
      project: browseProject.name,
      defaultValue: 'Search in {{project}}...',
    })
    : t('projects.search_placeholder_inbox', { defaultValue: 'Search Inbox...' });
  const summaryChips: ProjectSummaryChip[] = [
    {
      key: 'items',
      label: t('projects.summary_items', { defaultValue: 'Items' }),
      value: String(workspaceQueryResult.summary.totalItems),
      testId: 'projects-summary-total-items',
    },
    {
      key: 'duration',
      label: t('projects.summary_duration', { defaultValue: 'Total duration' }),
      value: formatSummaryDuration(workspaceQueryResult.summary.totalDuration, t),
      testId: 'projects-summary-total-duration',
    },
    {
      key: 'latest',
      label: t('projects.summary_latest_activity', { defaultValue: 'Latest activity' }),
      value: workspaceQueryResult.summary.latestTimestamp
        ? formatTimestamp(workspaceQueryResult.summary.latestTimestamp)
        : t('projects.summary_no_activity', { defaultValue: 'No activity yet' }),
      testId: 'projects-summary-latest-activity',
    },
    {
      key: 'type-split',
      label: t('projects.summary_type_split', { defaultValue: 'Type split' }),
      value: t('projects.summary_type_split_value', {
        recordings: workspaceQueryResult.summary.recordingCount,
        imports: workspaceQueryResult.summary.batchCount,
        defaultValue: `${workspaceQueryResult.summary.recordingCount} recordings / ${workspaceQueryResult.summary.batchCount} imports`,
      }),
      testId: 'projects-summary-type-split',
    },
  ];

  return {
    browseScope,
    setBrowseScope,
    isAllItemsScope,
    isInboxScope,
    browseProjectId,
    browseProject,
    isFilterMenuOpen,
    setIsFilterMenuOpen,
    searchQuery,
    setSearchQuery,
    activeSearchResultId,
    setActiveSearchResultId,
    filterType,
    setFilterType,
    dateFilter,
    setDateFilter,
    sortOrder,
    setSortOrder,
    isScrolled,
    handleScroll,
    scopedItems,
    filteredAndSortedItems,
    searchMatchByItemId,
    itemCounts,
    headerTitle,
    headerDescription,
    showWorkflowActions,
    headerIcon,
    searchInputLabel,
    summaryChips,
    moveOptions,
    filterTypeOptions,
    dateFilterOptions,
    sortOptions,
    activeFilterCount,
    hasActiveFilters,
    filterPopoverHint,
    handleWorkspaceSearchInputKeyDown,
    resetBrowseState,
  };
}
