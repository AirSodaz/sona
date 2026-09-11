import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { HistoryWorkspaceItemCounts, HistoryWorkspaceSummary } from '../../../bindings';
import { historyQueryWorkspace } from '../../../services/tauri/history';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import { logger } from '../../../utils/logger';
import type {
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
  WorkspaceQueryRequest,
  WorkspaceQueryResult,
} from '../types';

export const EMPTY_WORKSPACE_QUERY_RESULT: WorkspaceQueryResult = {
  filteredItems: [],
  searchMatchByItemId: {},
  filteredItemCount: 0,
  hasMore: false,
  summary: {
    totalItems: 0,
    totalDuration: 0,
    latestTimestamp: null,
    recordingCount: 0,
    batchCount: 0,
  },
  itemCounts: {
    untagged: 0,
    trash: 0,
    byTagId: {},
  },
};

export function deriveFallbackItemCounts(
  historyItems: HistoryItemType[]
): HistoryWorkspaceItemCounts {
  const byTagId: Record<string, number> = {};
  let untagged = 0;
  let trash = 0;
  for (const item of historyItems) {
    if (item.deletedAt != null) {
      trash += 1;
    } else if (!item.projectId) {
      untagged += 1;
    } else {
      byTagId[item.projectId] = (byTagId[item.projectId] || 0) + 1;
    }
  }
  return {
    untagged,
    inbox: untagged,
    trash,
    byTagId,
    byProjectId: byTagId,
  };
}

export function deriveFallbackSummary(
  historyItems: HistoryItemType[],
  scope: WorkspaceQueryRequest['scope']
): { summary: HistoryWorkspaceSummary; filteredItemCount: number } {
  let totalItems = 0;
  let totalDuration = 0;
  let latestTimestamp: number | null = null;
  let recordingCount = 0;
  let batchCount = 0;

  for (const item of historyItems) {
    let matchesScope = false;
    switch (scope.kind) {
      case 'all':
        matchesScope = item.deletedAt == null;
        break;
      case 'inbox':
        matchesScope = item.deletedAt == null && !item.projectId;
        break;
      case 'trash':
        matchesScope = item.deletedAt != null;
        break;
      case 'project':
        matchesScope = item.deletedAt == null && item.projectId === scope.projectId;
        break;
    }

    if (matchesScope) {
      totalItems += 1;
      totalDuration += Number(item.duration || 0);
      const timestamp = Number(item.timestamp || 0);
      if (latestTimestamp === null || timestamp > latestTimestamp) {
        latestTimestamp = timestamp;
      }
      if (item.type === 'batch') {
        batchCount += 1;
      } else {
        recordingCount += 1;
      }
    }
  }

  return {
    filteredItemCount: totalItems,
    summary: {
      totalItems,
      totalDuration,
      latestTimestamp,
      recordingCount,
      batchCount,
    },
  };
}

const WORKSPACE_QUERY_PAGE_SIZE = 100;

export interface WorkspaceQueryState extends WorkspaceQueryResult {
  initialLoadError: boolean;
  isInitialLoading: boolean;
  isLoadingMore: boolean;
  loadMoreError: boolean;
  loadMore: () => Promise<void>;
  retryInitialLoad: () => void;
}

interface WorkspaceQueryIdentity {
  historyItems: HistoryItemType[];
  request: Omit<WorkspaceQueryRequest, 'limit' | 'offset'>;
}

interface WorkspaceQuerySnapshot extends WorkspaceQueryIdentity {
  result: WorkspaceQueryResult;
}

interface UseWorkspaceQueryParams {
  dateFilter: ProjectDateFilter;
  filterType: ProjectFilterType;
  historyItems: HistoryItemType[];
  scope: WorkspaceQueryRequest['scope'];
  searchQuery: string;
  sortOrder: ProjectSortOrder;
}

export function useWorkspaceQuery({
  dateFilter,
  filterType,
  historyItems,
  scope,
  searchQuery,
  sortOrder,
}: UseWorkspaceQueryParams): WorkspaceQueryState {
  const [snapshot, setSnapshot] = useState<WorkspaceQuerySnapshot | null>(null);
  const [initialLoadFailure, setInitialLoadFailure] = useState<WorkspaceQueryIdentity | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState(false);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const requestIdRef = useRef(0);
  const loadingMoreRef = useRef(false);
  const request = useMemo<Omit<WorkspaceQueryRequest, 'limit' | 'offset'>>(
    () => ({
      scope,
      query: searchQuery,
      filterType,
      dateFilter,
      sortOrder,
    }),
    [dateFilter, filterType, scope, searchQuery, sortOrder]
  );
  const fallbackItemCounts = useMemo(() => deriveFallbackItemCounts(historyItems), [historyItems]);

  const activeItemCounts =
    snapshot?.result.itemCounts ??
    (historyItems.length > 0 ? fallbackItemCounts : EMPTY_WORKSPACE_QUERY_RESULT.itemCounts);

  const fallbackSummary = useMemo(
    () => deriveFallbackSummary(historyItems, scope),
    [historyItems, scope]
  );

  const hasCurrentSnapshot =
    snapshot?.request === request && snapshot.historyItems === historyItems;
  const initialLoadError =
    initialLoadFailure?.request === request && initialLoadFailure.historyItems === historyItems;

  const queryResult = useMemo<WorkspaceQueryResult>(() => {
    if (hasCurrentSnapshot) {
      return snapshot.result;
    }
    if (initialLoadError) {
      return {
        ...EMPTY_WORKSPACE_QUERY_RESULT,
        itemCounts: activeItemCounts,
      };
    }
    const isUnfilteredScope = !searchQuery.trim() && filterType === 'all' && dateFilter === 'all';
    return {
      filteredItems: [],
      searchMatchByItemId: {},
      filteredItemCount: isUnfilteredScope ? fallbackSummary.filteredItemCount : 0,
      hasMore: false,
      summary:
        historyItems.length > 0 ? fallbackSummary.summary : EMPTY_WORKSPACE_QUERY_RESULT.summary,
      itemCounts: activeItemCounts,
    };
  }, [
    activeItemCounts,
    dateFilter,
    fallbackSummary,
    filterType,
    hasCurrentSnapshot,
    historyItems.length,
    initialLoadError,
    searchQuery,
    snapshot,
  ]);
  const isInitialLoading = !hasCurrentSnapshot && !initialLoadError;

  useEffect(() => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    loadingMoreRef.current = false;

    void historyQueryWorkspace({
      ...request,
      limit: WORKSPACE_QUERY_PAGE_SIZE,
      offset: 0,
    })
      .then((result) => {
        if (requestIdRef.current === requestId) {
          setSnapshot({ historyItems, request, result });
          setInitialLoadFailure(null);
          setIsLoadingMore(false);
          setLoadMoreError(false);
        }
      })
      .catch(() => {
        logger.debug('[WorkspaceQuery] Initial query failed');
        if (requestIdRef.current === requestId) {
          setInitialLoadFailure({ historyItems, request });
          setIsLoadingMore(false);
          setLoadMoreError(false);
        }
      });
  }, [historyItems, request, retryAttempt]);

  const retryInitialLoad = useCallback(() => {
    setInitialLoadFailure(null);
    setRetryAttempt((attempt) => attempt + 1);
  }, []);

  const loadMore = useCallback(async () => {
    if (!hasCurrentSnapshot || !queryResult.hasMore || loadingMoreRef.current) {
      return;
    }

    const requestId = requestIdRef.current;
    const offset = queryResult.filteredItems.length;
    loadingMoreRef.current = true;
    setIsLoadingMore(true);
    setLoadMoreError(false);

    try {
      const nextPage = await historyQueryWorkspace({
        ...request,
        limit: WORKSPACE_QUERY_PAGE_SIZE,
        offset,
      });
      if (requestIdRef.current !== requestId) {
        return;
      }

      setSnapshot((current) => {
        if (current?.request !== request || current.historyItems !== historyItems) {
          return current;
        }
        const existingIds = new Set(current.result.filteredItems.map((item) => item.id));
        const nextItems = nextPage.filteredItems.filter((item) => !existingIds.has(item.id));
        return {
          ...current,
          result: {
            ...nextPage,
            filteredItems: [...current.result.filteredItems, ...nextItems],
            searchMatchByItemId: {
              ...current.result.searchMatchByItemId,
              ...nextPage.searchMatchByItemId,
            },
          },
        };
      });
    } catch {
      if (requestIdRef.current === requestId) {
        logger.debug('[WorkspaceQuery] Next page failed');
        setLoadMoreError(true);
      }
    } finally {
      if (requestIdRef.current === requestId) {
        loadingMoreRef.current = false;
        setIsLoadingMore(false);
      }
    }
  }, [
    hasCurrentSnapshot,
    historyItems,
    queryResult.filteredItems.length,
    queryResult.hasMore,
    request,
  ]);

  return {
    ...queryResult,
    initialLoadError,
    isInitialLoading,
    isLoadingMore: hasCurrentSnapshot && isLoadingMore,
    loadMoreError: hasCurrentSnapshot && loadMoreError,
    loadMore,
    retryInitialLoad,
  };
}
