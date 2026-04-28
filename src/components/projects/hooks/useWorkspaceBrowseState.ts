import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import { useDialogStore } from '../../../stores/dialogStore';
import { useErrorDialogStore } from '../../../stores/errorDialogStore';
import { getWorkspaceSearchResultDomId, matchWorkspaceItem } from '../../../utils/workspaceSearch';
import {
  ALL_ITEMS_SCOPE,
  DEFAULT_DATE_FILTER,
  DEFAULT_FILTER_TYPE,
  DEFAULT_SORT_ORDER,
  INBOX_SCOPE,
} from '../constants';
import type {
  FilteredProjectItemEntry,
  ProjectBrowseScope,
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
  ProjectSummary,
  ProjectSummaryChip,
  TranslationFn,
} from '../types';
import {
  compareProjectItems,
  formatSummaryDuration,
  formatTimestamp,
  matchesDateFilter,
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
  const [browseScope, setBrowseScope] = useState<ProjectBrowseScope>(() => activeProjectId || INBOX_SCOPE);
  const [isFilterMenuOpen, setIsFilterMenuOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [activeSearchResultId, setActiveSearchResultId] = useState<string | null>(null);
  const [filterType, setFilterType] = useState<ProjectFilterType>(DEFAULT_FILTER_TYPE);
  const [dateFilter, setDateFilter] = useState<ProjectDateFilter>(DEFAULT_DATE_FILTER);
  const [sortOrder, setSortOrder] = useState<ProjectSortOrder>(DEFAULT_SORT_ORDER);
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

  const isAllItemsScope = browseScope === ALL_ITEMS_SCOPE;
  const isInboxScope = browseScope === INBOX_SCOPE;
  const browseProjectId = !isAllItemsScope && !isInboxScope ? browseScope : null;
  const browseProject = useMemo(
    () => projects.find((item) => item.id === browseProjectId) || null,
    [browseProjectId, projects],
  );

  useEffect(() => {
    setSearchQuery('');
    setFilterType(DEFAULT_FILTER_TYPE);
    setDateFilter(DEFAULT_DATE_FILTER);
    setIsFilterMenuOpen(false);
  }, [browseScope]);

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
  }, [filterMenuRef, isFilterMenuOpen]);

  useEffect(() => {
    if (browseScope === ALL_ITEMS_SCOPE || browseScope === INBOX_SCOPE) {
      return;
    }

    if (projects.some((item) => item.id === browseScope)) {
      return;
    }

    setBrowseScope(activeProjectId || INBOX_SCOPE);
  }, [activeProjectId, browseScope, projects]);

  const scopedItems = useMemo(
    () => historyItems.filter((item) => {
      if (isAllItemsScope) {
        return true;
      }

      if (isInboxScope) {
        return item.projectId === null;
      }

      return item.projectId === browseProjectId;
    }),
    [browseProjectId, historyItems, isAllItemsScope, isInboxScope],
  );

  const filteredItemEntries = useMemo<FilteredProjectItemEntry[]>(() => {
    const hasQuery = searchQuery.trim().length > 0;

    return scopedItems.flatMap((item) => {
      const searchMatch = hasQuery ? matchWorkspaceItem(item, searchQuery) : null;
      if (hasQuery && !searchMatch) {
        return [];
      }

      if (filterType !== 'all' && (item.type || 'recording') !== filterType) {
        return [];
      }

      if (!matchesDateFilter(item, dateFilter)) {
        return [];
      }

      return [{ item, searchMatch }];
    });
  }, [dateFilter, filterType, scopedItems, searchQuery]);

  const filteredAndSortedItemEntries = useMemo(
    () => [...filteredItemEntries].sort((a, b) => compareProjectItems(a.item, b.item, sortOrder)),
    [filteredItemEntries, sortOrder],
  );

  const filteredAndSortedItems = useMemo(
    () => filteredAndSortedItemEntries.map(({ item }) => item),
    [filteredAndSortedItemEntries],
  );

  const searchMatchByItemId = useMemo(
    () => new Map(filteredAndSortedItemEntries.map(({ item, searchMatch }) => [item.id, searchMatch])),
    [filteredAndSortedItemEntries],
  );

  const projectSummary = useMemo<ProjectSummary>(() => {
    let totalDuration = 0;
    let recordingCount = 0;
    let batchCount = 0;
    let latestTimestamp: number | null = null;

    scopedItems.forEach((item) => {
      totalDuration += item.duration || 0;
      latestTimestamp = latestTimestamp === null ? item.timestamp : Math.max(latestTimestamp, item.timestamp);

      if ((item.type || 'recording') === 'batch') {
        batchCount += 1;
        return;
      }

      recordingCount += 1;
    });

    return {
      totalItems: scopedItems.length,
      totalDuration,
      latestTimestamp,
      recordingCount,
      batchCount,
    };
  }, [scopedItems]);

  const itemCounts = useMemo(() => {
    const counts = new Map<string | null, number>();
    historyItems.forEach((item) => {
      const key = item.projectId ?? null;
      counts.set(key, (counts.get(key) || 0) + 1);
    });
    return counts;
  }, [historyItems]);

  useEffect(() => {
    setActiveSearchResultId(null);
  }, [browseScope, dateFilter, filterType, searchQuery, sortOrder]);

  useEffect(() => {
    if (!isSelectionMode) {
      return;
    }

    setActiveSearchResultId(null);
  }, [isSelectionMode]);

  useEffect(() => {
    if (!activeSearchResultId) {
      return;
    }

    if (filteredAndSortedItems.some((item) => item.id === activeSearchResultId)) {
      return;
    }

    setActiveSearchResultId(null);
  }, [activeSearchResultId, filteredAndSortedItems]);

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
  }, [filteredAndSortedItems]);

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
    searchInputRef,
    searchQuery,
  ]);

  const resetBrowseState = useCallback(() => {
    setSearchQuery('');
    setFilterType(DEFAULT_FILTER_TYPE);
    setDateFilter(DEFAULT_DATE_FILTER);
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
      value: String(projectSummary.totalItems),
      testId: 'projects-summary-total-items',
    },
    {
      key: 'duration',
      label: t('projects.summary_duration', { defaultValue: 'Total duration' }),
      value: formatSummaryDuration(projectSummary.totalDuration, t),
      testId: 'projects-summary-total-duration',
    },
    {
      key: 'latest',
      label: t('projects.summary_latest_activity', { defaultValue: 'Latest activity' }),
      value: projectSummary.latestTimestamp
        ? formatTimestamp(projectSummary.latestTimestamp)
        : t('projects.summary_no_activity', { defaultValue: 'No activity yet' }),
      testId: 'projects-summary-latest-activity',
    },
    {
      key: 'type-split',
      label: t('projects.summary_type_split', { defaultValue: 'Type split' }),
      value: t('projects.summary_type_split_value', {
        recordings: projectSummary.recordingCount,
        imports: projectSummary.batchCount,
        defaultValue: `${projectSummary.recordingCount} recordings / ${projectSummary.batchCount} imports`,
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
