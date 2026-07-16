import { useCallback, useMemo, useState } from 'react';
import type { ProjectRecord } from '../../../types/project';
import {
  ALL_ITEMS_SCOPE,
  DEFAULT_DATE_FILTER,
  DEFAULT_FILTER_TYPE,
  DEFAULT_SORT_ORDER,
  TRASH_SCOPE,
  UNTAGGED_SCOPE,
} from '../constants';
import type {
  ProjectBrowseScope,
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
} from '../types';

interface UseWorkspaceBrowseControlsParams {
  activeProjectId: string | null;
  projects: ProjectRecord[];
}

export function useWorkspaceBrowseControls({
  activeProjectId,
  projects,
}: UseWorkspaceBrowseControlsParams) {
  const [browseScopeState, setBrowseScopeState] = useState<ProjectBrowseScope>(() => activeProjectId || UNTAGGED_SCOPE);
  const [isFilterMenuOpen, setIsFilterMenuOpenState] = useState(false);
  const [searchQuery, setSearchQueryState] = useState('');
  const [activeSearchResultIdState, setActiveSearchResultIdState] = useState<string | null>(null);
  const [filterType, setFilterTypeState] = useState<ProjectFilterType>(DEFAULT_FILTER_TYPE);
  const [dateFilter, setDateFilterState] = useState<ProjectDateFilter>(DEFAULT_DATE_FILTER);
  const [sortOrder, setSortOrderState] = useState<ProjectSortOrder>(DEFAULT_SORT_ORDER);

  const browseScope = useMemo<ProjectBrowseScope>(() => {
    if (
      browseScopeState === ALL_ITEMS_SCOPE
      || browseScopeState === UNTAGGED_SCOPE
      || browseScopeState === TRASH_SCOPE
    ) {
      return browseScopeState;
    }

    if (projects.some((item) => item.id === browseScopeState)) {
      return browseScopeState;
    }

    return activeProjectId || UNTAGGED_SCOPE;
  }, [activeProjectId, browseScopeState, projects]);

  const isAllItemsScope = browseScope === ALL_ITEMS_SCOPE;
  const isInboxScope = browseScope === UNTAGGED_SCOPE;
  const isTrashScope = browseScope === TRASH_SCOPE;
  const browseProjectId = !isAllItemsScope && !isInboxScope && !isTrashScope ? browseScope : null;
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

  const resetBrowseState = useCallback(() => {
    setSearchQueryState('');
    setFilterTypeState(DEFAULT_FILTER_TYPE);
    setDateFilterState(DEFAULT_DATE_FILTER);
    setActiveSearchResultIdState(null);
  }, []);

  return {
    activeSearchResultIdState,
    browseProject,
    browseProjectId,
    browseScope,
    dateFilter,
    filterType,
    isAllItemsScope,
    isFilterMenuOpen,
    isInboxScope,
    isTrashScope,
    resetBrowseState,
    searchQuery,
    setActiveSearchResultId,
    setBrowseScope,
    setDateFilter,
    setFilterType,
    setIsFilterMenuOpen,
    setSearchQuery,
    setSortOrder,
    sortOrder,
  };
}
