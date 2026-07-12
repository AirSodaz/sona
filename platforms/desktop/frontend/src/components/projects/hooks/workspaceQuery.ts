import { useEffect, useMemo, useRef, useState } from 'react';
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
  scopedItems: [],
  scopedItemIds: [],
  searchMatchByItemId: {},
  summary: {
    totalItems: 0,
    totalDuration: 0,
    latestTimestamp: null,
    recordingCount: 0,
    batchCount: 0,
  },
  itemCounts: {
    inbox: 0,
    byProjectId: {},
  },
};

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
}: UseWorkspaceQueryParams): WorkspaceQueryResult {
  const [queryResult, setQueryResult] = useState<WorkspaceQueryResult>(EMPTY_WORKSPACE_QUERY_RESULT);
  const requestIdRef = useRef(0);
  const request = useMemo<WorkspaceQueryRequest>(() => ({
    scope,
    query: searchQuery,
    filterType,
    dateFilter,
    sortOrder,
  }), [dateFilter, filterType, scope, searchQuery, sortOrder]);

  useEffect(() => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    void historyQueryWorkspace(request)
      .then((result) => {
        if (requestIdRef.current !== requestId) {
          return;
        }

        setQueryResult(result);
      })
      .catch(() => {
        logger.debug('[WorkspaceQuery] Query failed, using empty result');
        if (requestIdRef.current !== requestId) {
          return;
        }

        setQueryResult(EMPTY_WORKSPACE_QUERY_RESULT);
      });
  }, [historyItems, request]);

  return queryResult;
}
