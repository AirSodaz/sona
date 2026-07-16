import type React from 'react';
import type { ProjectRecord } from '../../../types/project';
import {
  ALL_ITEMS_SCOPE,
  DEFAULT_DATE_FILTER,
  DEFAULT_FILTER_TYPE,
  TRASH_SCOPE,
  UNTAGGED_SCOPE,
} from '../constants';
import type {
  ProjectBrowseScope,
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSummaryChip,
  TranslationFn,
  WorkspaceQueryResult,
} from '../types';
import {
  formatSummaryDuration,
  formatTimestamp,
  renderScopeIcon,
} from '../utils';

interface WorkspaceViewModelParams {
  browseProject: ProjectRecord | null;
  browseScope: ProjectBrowseScope;
  dateFilter: ProjectDateFilter;
  filterType: ProjectFilterType;
  projects: ProjectRecord[];
  queryResult: WorkspaceQueryResult;
  t: TranslationFn;
}

export interface WorkspaceViewModel {
  activeFilterCount: number;
  dateFilterOptions: Array<{ value: ProjectDateFilter; label: string }>;
  filterPopoverHint: string;
  filterTypeOptions: Array<{ value: ProjectFilterType; label: string }>;
  hasActiveFilters: boolean;
  headerDescription: string;
  headerIcon: React.ReactNode;
  headerTitle: string;
  itemCounts: Map<string | null, number>;
  moveOptions: Array<{ value: string; label: string }>;
  searchInputLabel: string;
  showWorkflowActions: boolean;
  sortOptions: Array<{
    value: 'newest' | 'oldest' | 'duration_desc' | 'duration_asc' | 'title_asc';
    label: string;
  }>;
  summaryChips: ProjectSummaryChip[];
}

export function buildWorkspaceViewModel({
  browseProject,
  browseScope,
  dateFilter,
  filterType,
  projects,
  queryResult,
  t,
}: WorkspaceViewModelParams): WorkspaceViewModel {
  const isAllItemsScope = browseScope === ALL_ITEMS_SCOPE;
  const itemCounts = new Map<string | null, number>();
  itemCounts.set(null, queryResult.itemCounts.untagged ?? queryResult.itemCounts.inbox ?? 0);
  itemCounts.set(TRASH_SCOPE, queryResult.itemCounts.trash ?? 0);
  Object.entries(queryResult.itemCounts.byTagId ?? queryResult.itemCounts.byProjectId ?? {}).forEach(([tagId, count]) => {
    itemCounts.set(tagId, count);
  });

  const moveOptions = [
    { value: UNTAGGED_SCOPE, label: t('projects.untagged', { defaultValue: 'Untagged' }) },
    ...projects.map((project) => ({ value: project.id, label: project.name })),
  ];

  const filterTypeOptions: Array<{ value: ProjectFilterType; label: string }> = [
    { value: 'all', label: t('projects.filter_all_types', { defaultValue: 'All types' }) },
    { value: 'recording', label: t('projects.filter_recordings', { defaultValue: 'Recordings' }) },
    { value: 'batch', label: t('projects.filter_batch', { defaultValue: 'Batch imports' }) },
  ];

  const dateFilterOptions: Array<{ value: ProjectDateFilter; label: string }> = [
    { value: 'all', label: t('projects.date_all', { defaultValue: 'Any time' }) },
    { value: 'today', label: t('projects.date_today', { defaultValue: 'Today' }) },
    { value: 'week', label: t('projects.date_week', { defaultValue: 'Last 7 days' }) },
    { value: 'month', label: t('projects.date_month', { defaultValue: 'Last 30 days' }) },
  ];

  const sortOptions = [
    { value: 'newest' as const, label: t('projects.sort_newest', { defaultValue: 'Newest first' }) },
    { value: 'oldest' as const, label: t('projects.sort_oldest', { defaultValue: 'Oldest first' }) },
    { value: 'duration_desc' as const, label: t('projects.sort_duration_desc', { defaultValue: 'Longest first' }) },
    { value: 'duration_asc' as const, label: t('projects.sort_duration_asc', { defaultValue: 'Shortest first' }) },
    { value: 'title_asc' as const, label: t('projects.sort_title_asc', { defaultValue: 'Title A-Z' }) },
  ];

  const activeFilterLabels: string[] = [];
  if (filterType !== DEFAULT_FILTER_TYPE) {
    const typeLabel = filterTypeOptions.find((option) => option.value === filterType)?.label;
    if (typeLabel) {
      activeFilterLabels.push(typeLabel);
    }
  }

  if (dateFilter !== DEFAULT_DATE_FILTER) {
    const dateLabel = dateFilterOptions.find((option) => option.value === dateFilter)?.label;
    if (dateLabel) {
      activeFilterLabels.push(dateLabel);
    }
  }

  const activeFilterCount = activeFilterLabels.length;
  const hasActiveFilters = activeFilterCount > 0;
  const filterPopoverHint = hasActiveFilters
    ? activeFilterLabels.join(' · ')
    : t('projects.filter_menu_hint', {
      defaultValue: 'Refine the current workspace view by type or time.',
    });

  const headerTitle = isAllItemsScope
    ? t('projects.all_items', { defaultValue: 'All Items' })
    : browseScope === TRASH_SCOPE
    ? t('projects.trash', { defaultValue: 'Trash' })
    : browseProject?.name || t('projects.untagged', { defaultValue: 'Untagged' });
  const headerDescription = isAllItemsScope
    ? t('projects.all_items_description', {
      defaultValue: 'Browse everything across Inbox and your projects.',
    })
    : browseScope === TRASH_SCOPE
    ? t('projects.trash_description', {
      defaultValue: 'Restore items or delete them permanently.',
    })
    : browseProject
    ? browseProject.description
    : t('projects.untagged_description', {
      defaultValue: 'Untagged collects recordings and imports without a tag.',
    });
  const showWorkflowActions = !isAllItemsScope && browseScope !== TRASH_SCOPE;
  const headerIcon = renderScopeIcon(browseScope, browseProject);
  const searchInputLabel = isAllItemsScope
    ? t('projects.search_placeholder_all_items', { defaultValue: 'Search All Items...' })
    : browseScope === TRASH_SCOPE
    ? t('projects.search_placeholder_trash', { defaultValue: 'Search Trash...' })
    : browseProject
    ? t('projects.search_placeholder_project', {
      project: browseProject.name,
      defaultValue: 'Search in {{project}}...',
    })
    : t('projects.search_placeholder_untagged', { defaultValue: 'Search Untagged...' });
  const summaryChips: ProjectSummaryChip[] = [
    {
      key: 'items',
      label: t('projects.summary_items', { defaultValue: 'Items' }),
      value: String(queryResult.summary.totalItems),
      testId: 'projects-summary-total-items',
    },
    {
      key: 'duration',
      label: t('projects.summary_duration', { defaultValue: 'Total duration' }),
      value: formatSummaryDuration(queryResult.summary.totalDuration, t),
      testId: 'projects-summary-total-duration',
    },
    {
      key: 'latest',
      label: t('projects.summary_latest_activity', { defaultValue: 'Latest activity' }),
      value: queryResult.summary.latestTimestamp
        ? formatTimestamp(queryResult.summary.latestTimestamp)
        : t('projects.summary_no_activity', { defaultValue: 'No activity yet' }),
      testId: 'projects-summary-latest-activity',
    },
    {
      key: 'type-split',
      label: t('projects.summary_type_split', { defaultValue: 'Type split' }),
      value: t('projects.summary_type_split_value', {
        recordings: queryResult.summary.recordingCount,
        imports: queryResult.summary.batchCount,
        defaultValue: `${queryResult.summary.recordingCount} recordings / ${queryResult.summary.batchCount} imports`,
      }),
      testId: 'projects-summary-type-split',
    },
  ];

  return {
    activeFilterCount,
    dateFilterOptions,
    filterPopoverHint,
    filterTypeOptions,
    hasActiveFilters,
    headerDescription,
    headerIcon,
    headerTitle,
    itemCounts,
    moveOptions,
    searchInputLabel,
    showWorkflowActions,
    sortOptions,
    summaryChips,
  };
}
