import React from 'react';
import { CheckSquare, LayoutGrid, LayoutList, List, Search, SlidersHorizontal } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import { FolderIcon, XIcon } from '../Icons';
import type { ProjectDateFilter, ProjectFilterType, ProjectSortOrder, TranslationFn } from './types';

interface ProjectsToolbarProps {
  activeFilterCount: number;
  currentSearchResultId: string | null;
  dateFilter: ProjectDateFilter;
  dateFilterOptions: Array<{ value: string; label: string }>;
  filterMenuHint: string;
  filterMenuRef: React.RefObject<HTMLDivElement | null>;
  filterType: ProjectFilterType;
  filterTypeOptions: Array<{ value: string; label: string }>;
  filteredResultsCount: number;
  hasActiveFilters: boolean;
  isFilterMenuOpen: boolean;
  isSelectionMode: boolean;
  onClearSearch: () => void;
  onOpenHistoryFolder: () => void;
  onResetBrowseState: () => void;
  onSearchInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
  onSearchQueryChange: (value: string) => void;
  onSetDateFilter: (value: ProjectDateFilter) => void;
  onSetFilterMenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onSetFilterType: (value: ProjectFilterType) => void;
  onSetSortOrder: (value: ProjectSortOrder) => void;
  onSetViewMode: (value: 'list' | 'grid' | 'table') => void;
  onToggleSelectionMode: () => void;
  disableSelectionModeToggle?: boolean;
  scopedItemsCount: number;
  searchInputLabel: string;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  searchQuery: string;
  sortOptions: Array<{ value: string; label: string }>;
  sortOrder: ProjectSortOrder;
  t: TranslationFn;
  viewMode: 'list' | 'grid' | 'table';
}

export function ProjectsToolbar({
  activeFilterCount,
  currentSearchResultId,
  dateFilter,
  dateFilterOptions,
  filterMenuHint,
  filterMenuRef,
  filterType,
  filterTypeOptions,
  filteredResultsCount,
  hasActiveFilters,
  isFilterMenuOpen,
  isSelectionMode,
  onClearSearch,
  onOpenHistoryFolder,
  onResetBrowseState,
  onSearchInputKeyDown,
  onSearchQueryChange,
  onSetDateFilter,
  onSetFilterMenuOpen,
  onSetFilterType,
  onSetSortOrder,
  onSetViewMode,
  onToggleSelectionMode,
  disableSelectionModeToggle = false,
  scopedItemsCount,
  searchInputLabel,
  searchInputRef,
  searchQuery,
  sortOptions,
  sortOrder,
  t,
  viewMode,
}: ProjectsToolbarProps): React.JSX.Element {
  return (
    <div className="projects-toolbar" data-testid="projects-toolbar-default">
      <div className="projects-toolbar-left">
        <div className="projects-search">
          <Search size={16} className="projects-search-icon" />
          <input
            ref={searchInputRef}
            type="text"
            placeholder={searchInputLabel}
            aria-label={searchInputLabel}
            aria-activedescendant={currentSearchResultId ? `workspace-search-result-${currentSearchResultId}` : undefined}
            value={searchQuery}
            onChange={(event) => onSearchQueryChange(event.target.value)}
            onKeyDown={onSearchInputKeyDown}
          />
          {searchQuery && (
            <button
              type="button"
              className="btn btn-icon btn-text projects-search-clear"
              onClick={onClearSearch}
              aria-label={t('common.clear_search', { defaultValue: 'Clear search' })}
            >
              <XIcon width={14} height={14} />
            </button>
          )}
        </div>

        <div className="projects-filter-menu" ref={filterMenuRef}>
          <button
            type="button"
            className={`btn btn-icon projects-toolbar-icon projects-filter-trigger ${isFilterMenuOpen ? 'active' : ''} ${hasActiveFilters ? 'has-active' : ''}`}
            onClick={() => onSetFilterMenuOpen((value) => !value)}
            aria-haspopup="dialog"
            aria-label={t('projects.filter_button', { defaultValue: 'Filter' })}
            aria-expanded={isFilterMenuOpen}
            aria-controls="projects-filter-panel"
            data-tooltip={t('projects.filter_button', { defaultValue: 'Filter & Sort' })}
            data-tooltip-pos="bottom"
          >
            <SlidersHorizontal size={16} />
            {hasActiveFilters && (
              <span className="projects-filter-trigger-count" aria-hidden="true">
                {activeFilterCount}
              </span>
            )}
          </button>

          {isFilterMenuOpen && (
            <div
              id="projects-filter-panel"
              className="projects-filter-popover"
              role="dialog"
              aria-label={t('projects.filter_button', { defaultValue: 'Filter' })}
            >
              <div className="projects-filter-popover-header">
                <div className="projects-filter-popover-copy">
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: '8px' }}>
                    <strong>{t('projects.filter_button', { defaultValue: 'Filter & Sort' })}</strong>
                    <span className="projects-results-count" data-testid="projects-results-count-popover">
                      {t('projects.results_count', {
                        visible: filteredResultsCount,
                        total: scopedItemsCount,
                        defaultValue: `${filteredResultsCount} / ${scopedItemsCount}`,
                      })}
                    </span>
                  </div>
                  <span>{filterMenuHint}</span>
                </div>
                <button
                  type="button"
                  className="btn btn-text projects-filter-clear"
                  onClick={onResetBrowseState}
                  disabled={!hasActiveFilters}
                >
                  {t('projects.clear_filters', { defaultValue: 'Clear filters' })}
                </button>
              </div>

              <div className="projects-filter-popover-body">
                <div className="projects-filter-field">
                  <span className="projects-toolbar-field-label">
                    {t('projects.sort_label', { defaultValue: 'Sort items' })}
                  </span>
                  <Dropdown
                    value={sortOrder}
                    onChange={(value: string) => onSetSortOrder(value as ProjectSortOrder)}
                    options={sortOptions}
                    style={{ width: '100%' }}
                    aria-label={t('projects.sort_label', { defaultValue: 'Sort items' })}
                  />
                </div>
                <div className="projects-filter-field">
                  <span className="projects-toolbar-field-label">
                    {t('projects.filter_type_label', { defaultValue: 'Filter by type' })}
                  </span>
                  <Dropdown
                    value={filterType}
                    onChange={(value: string) => onSetFilterType(value as ProjectFilterType)}
                    options={filterTypeOptions}
                    style={{ width: '100%' }}
                    aria-label={t('projects.filter_type_label', { defaultValue: 'Filter by type' })}
                  />
                </div>
                <div className="projects-filter-field">
                  <span className="projects-toolbar-field-label">
                    {t('projects.filter_date_label', { defaultValue: 'Filter by date' })}
                  </span>
                  <Dropdown
                    value={dateFilter}
                    onChange={(value: string) => onSetDateFilter(value as ProjectDateFilter)}
                    options={dateFilterOptions}
                    style={{ width: '100%' }}
                    aria-label={t('projects.filter_date_label', { defaultValue: 'Filter by date' })}
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="projects-toolbar-right">
        <div className="projects-segmented-control">
          <div className="projects-view-toggles" role="group" aria-label={t('projects.view_mode', { defaultValue: 'View Mode' })}>
            <button
              type="button"
              className={`btn btn-icon projects-toolbar-icon ${viewMode === 'list' ? 'active' : ''}`}
              onClick={() => onSetViewMode('list')}
              aria-pressed={viewMode === 'list'}
              aria-label={t('projects.view_list', { defaultValue: 'List View' })}
              data-tooltip={t('projects.view_list', { defaultValue: 'List View' })}
              data-tooltip-pos="bottom"
            >
              <List size={16} />
            </button>
            <button
              type="button"
              className={`btn btn-icon projects-toolbar-icon ${viewMode === 'grid' ? 'active' : ''}`}
              onClick={() => onSetViewMode('grid')}
              aria-pressed={viewMode === 'grid'}
              aria-label={t('projects.view_grid', { defaultValue: 'Grid View' })}
              data-tooltip={t('projects.view_grid', { defaultValue: 'Grid View' })}
              data-tooltip-pos="bottom"
            >
              <LayoutGrid size={16} />
            </button>
            <button
              type="button"
              className={`btn btn-icon projects-toolbar-icon ${viewMode === 'table' ? 'active' : ''}`}
              onClick={() => onSetViewMode('table')}
              aria-pressed={viewMode === 'table'}
              aria-label={t('projects.view_table', { defaultValue: 'Table View' })}
              data-tooltip={t('projects.view_table', { defaultValue: 'Table View' })}
              data-tooltip-pos="bottom"
            >
              <LayoutList size={16} />
            </button>
          </div>

          <div className="projects-toolbar-divider" />

          <button
            type="button"
            className="btn btn-icon projects-toolbar-icon"
            onClick={onOpenHistoryFolder}
            aria-label={t('history.open_folder', { defaultValue: 'Open File Directory' })}
            data-tooltip={t('history.open_folder', { defaultValue: 'Open File Directory' })}
            data-tooltip-pos="bottom"
          >
            <FolderIcon width={16} height={16} />
          </button>
          <button
            type="button"
            className={`btn btn-icon projects-toolbar-icon ${isSelectionMode ? 'active' : ''}`}
            onClick={onToggleSelectionMode}
            aria-label={t('common.select', { defaultValue: 'Select' })}
            data-tooltip={t('common.select', { defaultValue: 'Select' })}
            data-tooltip-pos="bottom"
            disabled={disableSelectionModeToggle}
          >
            <CheckSquare size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}
