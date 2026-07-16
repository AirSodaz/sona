import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { HistoryItem as HistoryItemType } from '../../../types/history';
import { historyQueryWorkspace } from '../../../services/tauri/history';
import type {
  ProjectDateFilter,
  ProjectFilterType,
  ProjectSortOrder,
  WorkspaceQueryRequest,
  WorkspaceQueryResult,
} from '../types';
import { logger } from '../../../utils/logger';

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
  const request = useMemo<Omit<WorkspaceQueryRequest, 'limit' | 'offset'>>(() => ({
    scope,
    query: searchQuery,
    filterType,
    dateFilter,
    sortOrder,
  }), [dateFilter, filterType, scope, searchQuery, sortOrder]);
  const hasCurrentSnapshot = snapshot?.request === request && snapshot.historyItems === historyItems;
  const initialLoadError = initialLoadFailure?.request === request
    && initialLoadFailure.historyItems === historyItems;
  const queryResult = hasCurrentSnapshot ? snapshot.result : EMPTY_WORKSPACE_QUERY_RESULT;
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
  }, [hasCurrentSnapshot, historyItems, queryResult.filteredItems.length, queryResult.hasMore, request]);

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
