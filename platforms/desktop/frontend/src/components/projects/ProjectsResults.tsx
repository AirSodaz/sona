import React from 'react';
import { CircleAlert, Loader2, RefreshCw, Search } from 'lucide-react';
import {
  Virtuoso,
  VirtuosoGrid,
  type Components,
  type GridComponents,
  type VirtuosoGridHandle,
  type VirtuosoHandle,
} from 'react-virtuoso';
import { HistoryItem } from '../history/HistoryItem';
import { PlusCircleIcon } from '../Icons';
import type { HistoryItem as HistoryItemType } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import type { WorkspaceItemSearchMatch } from '../../utils/workspaceSearch';
import type { TranslationFn } from './types';
import type { ContextMenuOpenRequest } from '../context-menu/trigger';

interface ProjectsResultsProps {
  activeSearchResultId: string | null;
  browseProject: ProjectRecord | null;
  filteredAndSortedItems: HistoryItemType[];
  handleOpenItem: (item: HistoryItemType) => Promise<void>;
  initialLoadError: boolean;
  isAllItemsScope: boolean;
  isHistoryLoading: boolean;
  isInitialLoading: boolean;
  isLoadingMore: boolean;
  isSelectionMode: boolean;
  isTrashScope?: boolean;
  loadMoreError: boolean;
  lockedHistoryId: string | null;
  onDeleteHistoryItem: (id: string) => Promise<void>;
  onLoadMore: () => Promise<void>;
  onRenameHistoryItem: (id: string) => Promise<void>;
  onOpenHistoryContextMenu: (id: string, request: ContextMenuOpenRequest) => void;
  activeContextId: string | null;
  onRetryInitialLoad: () => void;
  onScroll?: React.UIEventHandler<HTMLDivElement>;
  onToggleSelection: (id: string) => void;
  resetBrowseState: () => void;
  filteredItemCount: number;
  scopeItemCount: number;
  searchMatchByItemId: Map<string, WorkspaceItemSearchMatch | null>;
  searchQuery: string;
  selectedHistoryId: string | null;
  selectedIds: string[];
  t: TranslationFn;
  viewMode: 'list' | 'grid' | 'table';
}

interface ProjectsVirtualContext {
  isSelectionMode: boolean;
  isLoadingMore: boolean;
  loadMoreError: boolean;
  onLoadMore: () => Promise<void>;
  showProjectBadge: boolean;
  t: TranslationFn;
  viewMode: 'list' | 'table';
}

const VIRTUAL_VIEWPORT_INCREASE = { top: 360, bottom: 720 };
const VIRTUAL_SCROLL_CLASS_NAME = 'projects-main-scroll projects-main-scroll--virtual';

function getVirtualScrollClassName(viewMode: ProjectsResultsProps['viewMode']): string {
  return `${VIRTUAL_SCROLL_CLASS_NAME} projects-results-scroll projects-results-scroll--${viewMode}`;
}

const ProjectsVirtualList = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & { context?: ProjectsVirtualContext }
>(function ProjectsVirtualList({ children, context, style, ...props }, ref) {
  const viewMode = context?.viewMode ?? 'list';
  const gutterClassName = viewMode === 'list' ? 'projects-layout-guttered' : '';

  return (
    <div
      {...props}
      ref={ref}
      className={`projects-list projects-layout-${viewMode} ${gutterClassName}`.trim()}
      style={style}
    >
      {children}
    </div>
  );
});

function ProjectsVirtualTopSpacer({ context }: { context?: ProjectsVirtualContext }): React.JSX.Element | null {
  if (context?.viewMode === 'table') {
    return null;
  }

  return <div className="projects-virtual-spacer projects-virtual-spacer--top" aria-hidden="true" />;
}

function ProjectsVirtualFooter({ context }: { context?: ProjectsVirtualContext }): React.JSX.Element {
  return (
    <>
      {context?.viewMode !== 'table' && (
        <div className="projects-virtual-spacer projects-virtual-spacer--bottom" aria-hidden="true" />
      )}
      {context?.isLoadingMore && (
        <div
          className="projects-pagination-footer"
          role="status"
          aria-label={context.t('projects.loading_more', { defaultValue: 'Loading more items' })}
        >
          <Loader2 size={16} className="queue-icon-spin" />
        </div>
      )}
      {context?.loadMoreError && (
        <div className="projects-pagination-footer">
          <button
            type="button"
            className="btn btn-icon btn-text"
            onClick={() => void context.onLoadMore()}
            aria-label={context.t('projects.retry_load_more', {
              defaultValue: 'Retry loading more items',
            })}
            data-tooltip={context.t('projects.retry_load_more', {
              defaultValue: 'Retry loading more items',
            })}
            data-tooltip-pos="top"
          >
            <RefreshCw size={16} />
          </button>
        </div>
      )}
    </>
  );
}

function ProjectsTableHeader({ context }: { context?: ProjectsVirtualContext }): React.JSX.Element | null {
  if (context?.viewMode !== 'table') {
    return null;
  }

  const { isSelectionMode, t } = context;
  const showProjectBadge = context.showProjectBadge;

  return (
    <div className="projects-table-header" role="row">
      {isSelectionMode && (
        <div
          className="projects-table-header-selection"
          role="columnheader"
          aria-label={t('projects.table_header_select', { defaultValue: 'Select' })}
        />
      )}
      <div className="projects-table-header-content">
        <div className="projects-table-header-cell projects-table-header-title" role="columnheader">
          {t('projects.table_header_name', { defaultValue: 'Name' })}
        </div>
        {showProjectBadge && (
          <div className="projects-table-header-cell projects-table-header-project" role="columnheader">
            {t('projects.table_header_tags', { defaultValue: 'Tags' })}
          </div>
        )}
        <div className="projects-table-header-meta">
          <div className="projects-table-header-cell projects-table-header-date" role="columnheader">
            {t('projects.table_header_date', { defaultValue: 'Date' })}
          </div>
          <div className="projects-table-header-cell projects-table-header-duration" role="columnheader">
            {t('projects.table_header_duration', { defaultValue: 'Duration' })}
          </div>
        </div>
      </div>
      {!isSelectionMode && (
        <div
          className="projects-table-header-actions"
          role="columnheader"
          aria-label={t('projects.table_header_actions', { defaultValue: 'Actions' })}
        />
      )}
    </div>
  );
}

function ProjectsVirtualHeader({ context }: { context?: ProjectsVirtualContext }): React.JSX.Element {
  return (
    <>
      <ProjectsVirtualTopSpacer context={context} />
      <ProjectsTableHeader context={context} />
    </>
  );
}

const PROJECTS_LIST_COMPONENTS: Components<HistoryItemType, ProjectsVirtualContext> = {
  Footer: ProjectsVirtualFooter,
  Header: ProjectsVirtualHeader,
  List: ProjectsVirtualList,
};

const PROJECTS_GRID_COMPONENTS: GridComponents = {
  Footer: ProjectsVirtualFooter,
  Header: ProjectsVirtualTopSpacer,
};

export function ProjectsResults({
  activeSearchResultId,
  browseProject,
  filteredAndSortedItems,
  handleOpenItem,
  initialLoadError,
  isAllItemsScope,
  isHistoryLoading,
  isInitialLoading,
  isLoadingMore,
  isSelectionMode,
  isTrashScope = false,
  loadMoreError,
  lockedHistoryId,
  onDeleteHistoryItem,
  onLoadMore,
  onRenameHistoryItem,
  onOpenHistoryContextMenu,
  activeContextId,
  onRetryInitialLoad,
  onScroll,
  onToggleSelection,
  resetBrowseState,
  filteredItemCount,
  scopeItemCount,
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
  const showProjectBadge = isAllItemsScope;
  const scrollClassName = React.useMemo(() => getVirtualScrollClassName(viewMode), [viewMode]);
  const listContext = React.useMemo<ProjectsVirtualContext>(() => ({
    isSelectionMode,
    isLoadingMore,
    loadMoreError,
    onLoadMore,
    showProjectBadge,
    t,
    viewMode: viewMode === 'table' ? 'table' : 'list',
  }), [isLoadingMore, isSelectionMode, loadMoreError, onLoadMore, showProjectBadge, t, viewMode]);
  const isLoading = isHistoryLoading || isInitialLoading;
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
    const isLockedHistoryItem = item.id === lockedHistoryId;

    return (
      <HistoryItem
        key={item.id}
        item={item}
        onLoad={handleOpenItem}
        onDelete={onDeleteHistoryItem}
        onRename={onRenameHistoryItem}
        onOpenContextMenu={onOpenHistoryContextMenu}
        isContextMenuOpen={activeContextId === `workspace:history:${item.id}`}
        isLoadDisabled={isTrashScope || (lockedHistoryId != null && !isLockedHistoryItem)}
        isRenameDisabled={isTrashScope || isLockedHistoryItem}
        isDeleteDisabled={isLockedHistoryItem}
        searchQuery={searchQuery}
        searchTitleMatch={searchMatch?.titleMatch ?? null}
        searchSnippet={searchMatch?.displaySnippet ?? null}
        isSelectionMode={isSelectionMode}
        isSelected={isSelectionMode ? selectedIdsSet.has(item.id) : selectedHistoryId === item.id}
        isKeyboardActive={!isSelectionMode && activeSearchResultId === item.id}
        onToggleSelection={onToggleSelection}
        layout={viewMode}
        showProjectBadge={showProjectBadge}
      />
    );
  }, [
    activeSearchResultId,
    handleOpenItem,
    isSelectionMode,
    isTrashScope,
    lockedHistoryId,
    onDeleteHistoryItem,
    onRenameHistoryItem,
    onOpenHistoryContextMenu,
    activeContextId,
    onToggleSelection,
    searchMatchByItemId,
    searchQuery,
    selectedHistoryId,
    selectedIdsSet,
    showProjectBadge,
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
      {!isLoading && initialLoadError && (
        renderScrollableState(<div className="projects-overview-card">
          <CircleAlert size={28} />
          <h4>
            {t('projects.initial_load_failed_title', {
              defaultValue: 'Workspace items could not be loaded',
            })}
          </h4>
          <p>
            {t('projects.initial_load_failed_hint', {
              defaultValue: 'Check the connection and try again.',
            })}
          </p>
          <button type="button" className="btn btn-secondary" onClick={onRetryInitialLoad}>
            <RefreshCw size={16} />
            {t('common.retry', { defaultValue: 'Retry' })}
          </button>
        </div>)
      )}

      {!isLoading && !initialLoadError && scopeItemCount === 0 && !searchQuery && (
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

      {!isLoading && !initialLoadError && (scopeItemCount > 0 || searchQuery) && filteredItemCount === 0 && (
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

      {isLoading && (
        renderScrollableState(<div className="projects-list-empty">
          {t('history.loading')}
        </div>)
      )}

      {!isLoading && filteredAndSortedItems.length > 0 && viewMode === 'grid' && (
        <VirtuosoGrid
          key={viewMode}
          className={scrollClassName}
          components={PROJECTS_GRID_COMPONENTS}
          computeItemKey={(_index, item) => item.id}
          context={listContext}
          data={filteredAndSortedItems}
          endReached={() => void onLoadMore()}
          increaseViewportBy={VIRTUAL_VIEWPORT_INCREASE}
          itemClassName="projects-grid-virtual-item"
          itemContent={renderVirtualItem}
          listClassName="projects-list projects-layout-grid projects-layout-guttered"
          onScroll={onScroll}
          ref={virtuosoGridRef}
        />
      )}

      {!isLoading && filteredAndSortedItems.length > 0 && viewMode !== 'grid' && (
        <Virtuoso
          key={viewMode}
          className={scrollClassName}
          components={PROJECTS_LIST_COMPONENTS}
          computeItemKey={(_index, item) => item.id}
          context={listContext}
          data={filteredAndSortedItems}
          defaultItemHeight={viewMode === 'table' ? 64 : 96}
          increaseViewportBy={VIRTUAL_VIEWPORT_INCREASE}
          itemContent={renderVirtualItem}
          endReached={() => void onLoadMore()}
          onScroll={onScroll}
          ref={virtuosoRef}
        />
      )}
    </>
  );
}
