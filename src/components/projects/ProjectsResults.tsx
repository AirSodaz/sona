import React from 'react';
import { Search } from 'lucide-react';
import { HistoryItem } from '../history/HistoryItem';
import { PlusCircleIcon } from '../Icons';
import type { HistoryItem as HistoryItemType } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import { matchWorkspaceItem } from '../../utils/workspaceSearch';
import type { TranslationFn } from './types';

interface ProjectsResultsProps {
  activeSearchResultId: string | null;
  browseProject: ProjectRecord | null;
  filteredAndSortedItems: HistoryItemType[];
  handleOpenItem: (item: HistoryItemType) => Promise<void>;
  isAllItemsScope: boolean;
  isHistoryLoading: boolean;
  isSelectionMode: boolean;
  onDeleteHistoryItem: (event: React.MouseEvent, id: string) => Promise<void>;
  onRenameHistoryItem: (event: React.MouseEvent, id: string) => Promise<void>;
  onToggleSelection: (id: string) => void;
  resetBrowseState: () => void;
  scopedItems: HistoryItemType[];
  searchMatchByItemId: Map<string, ReturnType<typeof matchWorkspaceItem>>;
  searchQuery: string;
  selectedHistoryId: string | null;
  selectedIds: string[];
  t: TranslationFn;
  viewMode: 'list' | 'grid' | 'table';
}

export function ProjectsResults({
  activeSearchResultId,
  browseProject,
  filteredAndSortedItems,
  handleOpenItem,
  isAllItemsScope,
  isHistoryLoading,
  isSelectionMode,
  onDeleteHistoryItem,
  onRenameHistoryItem,
  onToggleSelection,
  resetBrowseState,
  scopedItems,
  searchMatchByItemId,
  searchQuery,
  selectedHistoryId,
  selectedIds,
  t,
  viewMode,
}: ProjectsResultsProps): React.JSX.Element {
  return (
    <>
      {!isHistoryLoading && scopedItems.length === 0 && (
        <div className="projects-overview-card">
          <PlusCircleIcon />
          <h4>{t('projects.empty_state', { defaultValue: 'No items in this workspace yet.' })}</h4>
          <p>
            {isAllItemsScope
              ? t('projects.empty_all_items_hint', {
                defaultValue: 'Saved recordings and imports will appear here once you create some content.',
              })
              : browseProject
              ? t('projects.empty_project_hint', {
                defaultValue: 'Start a live recording or import files to begin building this project.',
              })
              : t('projects.empty_inbox_hint', {
                defaultValue: 'New recordings and imports will arrive here until you move them into a project.',
              })}
          </p>
        </div>
      )}

      {!isHistoryLoading && scopedItems.length > 0 && filteredAndSortedItems.length === 0 && (
        <div className="projects-overview-card">
          <Search size={28} />
          <h4>{t('projects.no_results_title', { defaultValue: 'No matching items' })}</h4>
          <p>
            {t('projects.no_results_hint', {
              defaultValue: 'Try a different search or clear the current filters.',
            })}
          </p>
          <button type="button" className="btn btn-secondary" onClick={resetBrowseState}>
            {t('projects.clear_filters', { defaultValue: 'Clear filters' })}
          </button>
        </div>
      )}

      {isHistoryLoading && (
        <div className="projects-list-empty">
          {t('history.loading')}
        </div>
      )}

      {!isHistoryLoading && filteredAndSortedItems.length > 0 && (
        <div className={`projects-list projects-layout-${viewMode}`}>
          {viewMode === 'table' && (
            <div className="projects-table-header" role="row">
              {isSelectionMode && <div role="columnheader" style={{ width: '40px' }} />}
              <div style={{ flex: 1, display: 'flex', alignItems: 'center', minWidth: 0 }}>
                <div className="projects-table-header-cell projects-table-header-title" role="columnheader">
                  {t('projects.table_header_name', { defaultValue: 'Name' })}
                </div>
                <div className="projects-table-header-cell projects-table-header-project" role="columnheader">
                  {t('projects.table_header_project', { defaultValue: 'Project' })}
                </div>
                <div style={{ flex: 2, display: 'flex', alignItems: 'center' }}>
                  <div className="projects-table-header-cell projects-table-header-date" role="columnheader">
                    {t('projects.table_header_date', { defaultValue: 'Date' })}
                  </div>
                  <div className="projects-table-header-cell projects-table-header-duration" role="columnheader">
                    {t('projects.table_header_duration', { defaultValue: 'Duration' })}
                  </div>
                </div>
              </div>
              {!isSelectionMode && <div role="columnheader" style={{ width: '48px' }} />}
            </div>
          )}
          {filteredAndSortedItems.map((item) => {
            const searchMatch = searchMatchByItemId.get(item.id) ?? null;
            return (
              <HistoryItem
                key={item.id}
                item={item}
                onLoad={handleOpenItem}
                onDelete={onDeleteHistoryItem}
                onRename={onRenameHistoryItem}
                searchQuery={searchQuery}
                searchTitleMatch={searchMatch?.titleMatch ?? null}
                searchSnippet={searchMatch?.displaySnippet ?? null}
                isSelectionMode={isSelectionMode}
                isSelected={isSelectionMode ? selectedIds.includes(item.id) : selectedHistoryId === item.id}
                isKeyboardActive={!isSelectionMode && activeSearchResultId === item.id}
                onToggleSelection={onToggleSelection}
                layout={viewMode}
              />
            );
          })}
        </div>
      )}
    </>
  );
}
