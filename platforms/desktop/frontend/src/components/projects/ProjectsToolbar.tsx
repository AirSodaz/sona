import { CheckSquare, LayoutGrid, LayoutList, List, SlidersHorizontal } from 'lucide-react';
import type React from 'react';
import { Dropdown } from '../Dropdown';
import { FolderIcon } from '../Icons';
import { Button, IconButton } from '../ui/Button';
import { SearchInput } from '../ui/SearchInput';
import type {
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
  TranslationFn,
} from './types';

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
        <SearchInput
          ref={searchInputRef}
          wrapperClassName="projects-search"
          value={searchQuery}
          onValueChange={onSearchQueryChange}
          onClear={onClearSearch}
          placeholder={searchInputLabel}
          aria-label={searchInputLabel}
          clearLabel={t('common.clear_search', { defaultValue: 'Clear search' })}
          aria-activedescendant={
            currentSearchResultId ? `workspace-search-result-${currentSearchResultId}` : undefined
          }
          onKeyDown={onSearchInputKeyDown}
        />

        <div className="projects-filter-menu" ref={filterMenuRef}>
          <Button
            tier="ghost"
            size="md"
            active={isFilterMenuOpen}
            className={`projects-filter-trigger ${hasActiveFilters ? 'has-active' : ''}`}
            onClick={() => onSetFilterMenuOpen((value) => !value)}
            aria-haspopup="dialog"
            aria-label={t('projects.filter_button', { defaultValue: 'Filter' })}
            aria-expanded={isFilterMenuOpen}
            aria-controls="projects-filter-panel"
            tooltip={t('projects.filter_button', { defaultValue: 'Filter & Sort' })}
            tooltipPos="bottom"
          >
            <SlidersHorizontal size={16} />
            {hasActiveFilters && (
              <span className="projects-filter-trigger-count" aria-hidden="true">
                {activeFilterCount}
              </span>
            )}
          </Button>

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
                    <strong>
                      {t('projects.filter_button', { defaultValue: 'Filter & Sort' })}
                    </strong>
                    <span
                      className="projects-results-count"
                      data-testid="projects-results-count-popover"
                    >
                      {t('projects.results_count', {
                        visible: filteredResultsCount,
                        total: scopedItemsCount,
                        defaultValue: `${filteredResultsCount} / ${scopedItemsCount}`,
                      })}
                    </span>
                  </div>
                  <span>{filterMenuHint}</span>
                </div>
                <Button
                  tier="ghost"
                  size="sm"
                  className="projects-filter-clear"
                  onClick={onResetBrowseState}
                  disabled={!hasActiveFilters}
                >
                  {t('projects.clear_filters', { defaultValue: 'Clear filters' })}
                </Button>
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
          <div
            className="projects-view-toggles"
            role="group"
            aria-label={t('projects.view_mode', { defaultValue: 'View Mode' })}
          >
            <IconButton
              tier="ghost"
              size="md"
              active={viewMode === 'list'}
              className="projects-toolbar-icon"
              icon={<List size={16} />}
              onClick={() => onSetViewMode('list')}
              aria-pressed={viewMode === 'list'}
              aria-label={t('projects.view_list', { defaultValue: 'List View' })}
              tooltipPos="bottom"
            />
            <IconButton
              tier="ghost"
              size="md"
              active={viewMode === 'grid'}
              className="projects-toolbar-icon"
              icon={<LayoutGrid size={16} />}
              onClick={() => onSetViewMode('grid')}
              aria-pressed={viewMode === 'grid'}
              aria-label={t('projects.view_grid', { defaultValue: 'Grid View' })}
              tooltipPos="bottom"
            />
            <IconButton
              tier="ghost"
              size="md"
              active={viewMode === 'table'}
              className="projects-toolbar-icon"
              icon={<LayoutList size={16} />}
              onClick={() => onSetViewMode('table')}
              aria-pressed={viewMode === 'table'}
              aria-label={t('projects.view_table', { defaultValue: 'Table View' })}
              tooltipPos="bottom"
            />
          </div>

          <div className="projects-toolbar-divider" />

          <IconButton
            tier="ghost"
            size="md"
            className="projects-toolbar-icon"
            icon={<FolderIcon width={16} height={16} />}
            onClick={onOpenHistoryFolder}
            aria-label={t('history.open_folder', { defaultValue: 'Open File Directory' })}
            tooltipPos="bottom"
          />
          <IconButton
            tier="ghost"
            size="md"
            active={isSelectionMode}
            className="projects-toolbar-icon"
            icon={<CheckSquare size={16} />}
            onClick={onToggleSelectionMode}
            aria-label={t('common.select', { defaultValue: 'Select' })}
            tooltipPos="bottom"
          />
        </div>
      </div>
    </div>
  );
}
