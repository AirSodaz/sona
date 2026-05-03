import React from 'react';
import { Search } from 'lucide-react';
import {
  Virtuoso,
  VirtuosoGrid,
  type Components,
  type VirtuosoGridHandle,
  type VirtuosoHandle,
} from 'react-virtuoso';
import { HistoryItem } from '../history/HistoryItem';
import { PlusCircleIcon } from '../Icons';
import type { HistoryItem as HistoryItemType } from '../../types/history';
import { isLiveRecordDraftHistoryItem } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import type { WorkspaceItemSearchMatch } from '../../utils/workspaceSearch';
import type { TranslationFn } from './types';

interface ProjectsResultsProps {
  activeSearchResultId: string | null;
  browseProject: ProjectRecord | null;
  filteredAndSortedItems: HistoryItemType[];
  handleOpenItem: (item: HistoryItemType) => Promise<void>;
  isHistoryInteractionLocked: boolean;
  isAllItemsScope: boolean;
  isHistoryLoading: boolean;
  isSelectionMode: boolean;
  onDeleteHistoryItem: (event: React.MouseEvent, id: string) => Promise<void>;
  onRenameHistoryItem: (event: React.MouseEvent, id: string) => Promise<void>;
  onScroll?: React.UIEventHandler<HTMLDivElement>;
  onToggleSelection: (id: string) => void;
  resetBrowseState: () => void;
  scopedItems: HistoryItemType[];
  searchMatchByItemId: Map<string, WorkspaceItemSearchMatch | null>;
  searchQuery: string;
  selectedHistoryId: string | null;
  selectedIds: string[];
  t: TranslationFn;
  viewMode: 'list' | 'grid' | 'table';
}

interface ProjectsVirtualContext {
  isSelectionMode: boolean;
  t: TranslationFn;
  viewMode: 'list' | 'table';
}

const VIRTUAL_VIEWPORT_INCREASE = { top: 360, bottom: 720 };

const ProjectsVirtualList = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & { context?: ProjectsVirtualContext }
>(function ProjectsVirtualList({ children, context, style, ...props }, ref) {
  const viewMode = context?.viewMode ?? 'list';

  return (
    <div
      {...props}
      ref={ref}
      className={`projects-list projects-layout-${viewMode}`}
      style={style}
    >
      {children}
    </div>
  );
});

function ProjectsTableHeader({ context }: { context?: ProjectsVirtualContext }): React.JSX.Element | null {
  if (context?.viewMode !== 'table') {
    return null;
  }

  const { isSelectionMode, t } = context;

  return (
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
  );
}

const PROJECTS_LIST_COMPONENTS: Components<HistoryItemType, ProjectsVirtualContext> = {
  Header: ProjectsTableHeader,
  List: ProjectsVirtualList,
};

export function ProjectsResults({
  activeSearchResultId,
  browseProject,
  filteredAndSortedItems,
  handleOpenItem,
  isHistoryInteractionLocked,
  isAllItemsScope,
  isHistoryLoading,
  isSelectionMode,
  onDeleteHistoryItem,
  onRenameHistoryItem,
  onScroll,
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
  const virtuosoRef = React.useRef<VirtuosoHandle | null>(null);
  const virtuosoGridRef = React.useRef<VirtuosoGridHandle | null>(null);
  const selectedIdsSet = React.useMemo(() => new Set(selectedIds), [selectedIds]);
  const listContext = React.useMemo<ProjectsVirtualContext>(() => ({
    isSelectionMode,
    t,
    viewMode: viewMode === 'table' ? 'table' : 'list',
  }), [isSelectionMode, t, viewMode]);
  const activeSearchResultIndex = React.useMemo(() => {
    if (!activeSearchResultId) {
      return -1;
    }

    return filteredAndSortedItems.findIndex((item) => item.id === activeSearchResultId);
  }, [activeSearchResultId, filteredAndSortedItems]);

  React.useEffect(() => {
    if (activeSearchResultIndex < 0) {
      return;
    }

    const location = {
      align: 'center' as const,
      behavior: 'smooth' as const,
      index: activeSearchResultIndex,
    };

    if (viewMode === 'grid') {
      virtuosoGridRef.current?.scrollToIndex(location);
      return;
    }

    virtuosoRef.current?.scrollToIndex(location);
  }, [activeSearchResultIndex, viewMode]);

  const renderHistoryItem = React.useCallback((item: HistoryItemType) => {
    const searchMatch = searchMatchByItemId.get(item.id) ?? null;
    const isLockedLiveDraft = isHistoryInteractionLocked && isLiveRecordDraftHistoryItem(item);

    return (
      <HistoryItem
        key={item.id}
        item={item}
        onLoad={handleOpenItem}
        onDelete={onDeleteHistoryItem}
        onRename={onRenameHistoryItem}
        isLoadDisabled={isHistoryInteractionLocked && !isLockedLiveDraft}
        isRenameDisabled={isLockedLiveDraft}
        isDeleteDisabled={isLockedLiveDraft}
        searchQuery={searchQuery}
        searchTitleMatch={searchMatch?.titleMatch ?? null}
        searchSnippet={searchMatch?.displaySnippet ?? null}
        isSelectionMode={isSelectionMode}
        isSelected={isSelectionMode ? selectedIdsSet.has(item.id) : selectedHistoryId === item.id}
        isKeyboardActive={!isSelectionMode && activeSearchResultId === item.id}
        onToggleSelection={onToggleSelection}
        layout={viewMode}
      />
    );
  }, [
    activeSearchResultId,
    handleOpenItem,
    isHistoryInteractionLocked,
    isSelectionMode,
    onDeleteHistoryItem,
    onRenameHistoryItem,
    onToggleSelection,
    searchMatchByItemId,
    searchQuery,
    selectedHistoryId,
    selectedIdsSet,
    viewMode,
  ]);

  const renderVirtualItem = React.useCallback((_index: number, item: HistoryItemType) => (
    renderHistoryItem(item)
  ), [renderHistoryItem]);

  const renderScrollableState = (children: React.ReactNode) => (
    <div className="projects-main-scroll" onScroll={onScroll}>
      {children}
    </div>
  );

  return (
    <>
      {!isHistoryLoading && scopedItems.length === 0 && (
        renderScrollableState(<div className="projects-overview-card">
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
        </div>)
      )}

      {!isHistoryLoading && scopedItems.length > 0 && filteredAndSortedItems.length === 0 && (
        renderScrollableState(<div className="projects-overview-card">
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
        </div>)
      )}

      {isHistoryLoading && (
        renderScrollableState(<div className="projects-list-empty">
          {t('history.loading')}
        </div>)
      )}

      {!isHistoryLoading && filteredAndSortedItems.length > 0 && viewMode === 'grid' && (
        <VirtuosoGrid
          className="projects-main-scroll"
          computeItemKey={(_index, item) => item.id}
          data={filteredAndSortedItems}
          increaseViewportBy={VIRTUAL_VIEWPORT_INCREASE}
          itemClassName="projects-grid-virtual-item"
          itemContent={renderVirtualItem}
          listClassName="projects-list projects-layout-grid"
          onScroll={onScroll}
          ref={virtuosoGridRef}
        />
      )}

      {!isHistoryLoading && filteredAndSortedItems.length > 0 && viewMode !== 'grid' && (
        <Virtuoso
          className="projects-main-scroll"
          components={PROJECTS_LIST_COMPONENTS}
          computeItemKey={(_index, item) => item.id}
          context={listContext}
          data={filteredAndSortedItems}
          defaultItemHeight={viewMode === 'table' ? 64 : 96}
          increaseViewportBy={VIRTUAL_VIEWPORT_INCREASE}
          itemContent={renderVirtualItem}
          onScroll={onScroll}
          ref={virtuosoRef}
        />
      )}
    </>
  );
}
