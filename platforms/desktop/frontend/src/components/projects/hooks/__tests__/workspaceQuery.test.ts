import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HistoryItem } from '../../../../types/history';
import { useWorkspaceQuery } from '../workspaceQuery';
import type { WorkspaceQueryResult } from '../../types';

const historyQueryWorkspaceMock = vi.hoisted(() => vi.fn());

vi.mock('../../../../services/tauri/history', () => ({
  historyQueryWorkspace: historyQueryWorkspaceMock,
}));

function item(id: string): HistoryItem {
  return {
    id,
    title: `Item ${id}`,
    timestamp: 1,
    duration: 1,
    audioPath: `${id}.wav`,
    transcriptPath: `${id}.json`,
    previewText: '',
    searchContent: '',
    type: 'recording',
    projectId: null,
  };
}

function page(items: HistoryItem[], filteredItemCount: number, hasMore: boolean) {
  return {
    filteredItems: items,
    searchMatchByItemId: Object.fromEntries(items.map((entry) => [entry.id, null])),
    filteredItemCount,
    hasMore,
    summary: {
      totalItems: filteredItemCount,
      totalDuration: filteredItemCount,
      latestTimestamp: 1,
      recordingCount: filteredItemCount,
      batchCount: 0,
    },
    itemCounts: { inbox: filteredItemCount, byProjectId: {} },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((promiseResolve) => {
    resolve = promiseResolve;
  });
  return { promise, resolve };
}

const baseParams = {
  dateFilter: 'all' as const,
  filterType: 'all' as const,
  historyItems: [] as HistoryItem[],
  scope: { kind: 'all' as const },
  searchQuery: '',
  sortOrder: 'newest' as const,
};

describe('useWorkspaceQuery', () => {
  beforeEach(() => {
    historyQueryWorkspaceMock.mockReset();
  });

  it('requests the first 100 workspace items from offset zero', async () => {
    historyQueryWorkspaceMock.mockResolvedValue(page([item('a')], 1, false));

    const { result } = renderHook(() => useWorkspaceQuery(baseParams));

    await waitFor(() => expect(result.current.filteredItems).toHaveLength(1));
    expect(historyQueryWorkspaceMock).toHaveBeenCalledWith(expect.objectContaining({
      limit: 100,
      offset: 0,
    }));
  });

  it('exposes an initial error and retries the first page without treating it as empty data', async () => {
    historyQueryWorkspaceMock
      .mockRejectedValueOnce(new Error('temporary failure'))
      .mockResolvedValueOnce(page([item('a')], 1, false));

    const { result } = renderHook(() => useWorkspaceQuery(baseParams));

    await waitFor(() => expect(result.current.initialLoadError).toBe(true));
    expect(result.current.isInitialLoading).toBe(false);
    expect(historyQueryWorkspaceMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      await result.current.retryInitialLoad();
    });

    expect(historyQueryWorkspaceMock).toHaveBeenNthCalledWith(2, expect.objectContaining({
      limit: 100,
      offset: 0,
    }));
    expect(result.current.initialLoadError).toBe(false);
    expect(result.current.filteredItems.map((entry) => entry.id)).toEqual(['a']);
  });

  it('loads and deduplicates the next page using the merged item count as offset', async () => {
    historyQueryWorkspaceMock
      .mockResolvedValueOnce(page([item('a'), item('b')], 3, true))
      .mockResolvedValueOnce(page([item('b'), item('c')], 3, false));

    const { result } = renderHook(() => useWorkspaceQuery(baseParams));
    await waitFor(() => expect(result.current.filteredItems).toHaveLength(2));

    await act(async () => {
      await result.current.loadMore();
    });

    expect(historyQueryWorkspaceMock).toHaveBeenNthCalledWith(2, expect.objectContaining({
      limit: 100,
      offset: 2,
    }));
    expect(result.current.filteredItems.map((entry) => entry.id)).toEqual(['a', 'b', 'c']);
    expect(result.current.hasMore).toBe(false);
  });

  it('discards an in-flight next page after the query identity changes', async () => {
    const stalePage = deferred<ReturnType<typeof page>>();
    historyQueryWorkspaceMock
      .mockResolvedValueOnce(page([item('initial')], 2, true))
      .mockReturnValueOnce(stalePage.promise)
      .mockResolvedValueOnce(page([item('current')], 1, false));

    const { result, rerender } = renderHook(
      (params: typeof baseParams) => useWorkspaceQuery(params),
      { initialProps: baseParams },
    );
    await waitFor(() => expect(result.current.filteredItems[0]?.id).toBe('initial'));

    let loadMorePromise!: Promise<void>;
    act(() => {
      loadMorePromise = result.current.loadMore();
    });
    await waitFor(() => expect(historyQueryWorkspaceMock).toHaveBeenCalledTimes(2));

    rerender({ ...baseParams, searchQuery: 'current' });
    await waitFor(() => expect(result.current.filteredItems[0]?.id).toBe('current'));

    await act(async () => {
      stalePage.resolve(page([item('stale')], 2, false));
      await loadMorePromise;
    });

    expect(result.current.filteredItems.map((entry) => entry.id)).toEqual(['current']);
  });

  it('retains loaded items and retries the same offset after a page fails', async () => {
    historyQueryWorkspaceMock
      .mockResolvedValueOnce(page([item('a')], 2, true))
      .mockRejectedValueOnce(new Error('temporary failure'))
      .mockResolvedValueOnce(page([item('b')], 2, false));

    const { result } = renderHook(() => useWorkspaceQuery(baseParams));
    await waitFor(() => expect(result.current.filteredItems).toHaveLength(1));

    await act(async () => {
      await result.current.loadMore();
    });
    expect(result.current.filteredItems.map((entry) => entry.id)).toEqual(['a']);
    expect(result.current.loadMoreError).toBe(true);

    await act(async () => {
      await result.current.loadMore();
    });
    expect(historyQueryWorkspaceMock).toHaveBeenNthCalledWith(3, expect.objectContaining({
      offset: 1,
    }));
    expect(result.current.filteredItems.map((entry) => entry.id)).toEqual(['a', 'b']);
    expect(result.current.loadMoreError).toBe(false);
  });

  it('retains stable itemCounts and scope summary without jumping to zero while switching scopes', async () => {
    const pendingQuery = deferred<WorkspaceQueryResult>();
    const items: HistoryItem[] = [
      { ...item('inbox-1'), projectId: null },
      { ...item('proj-1'), projectId: 'project-a' },
      { ...item('proj-2'), projectId: 'project-a' },
    ];

    historyQueryWorkspaceMock
      .mockResolvedValueOnce({
        ...page([items[0]], 1, false),
        itemCounts: {
          untagged: 1,
          trash: 0,
          byTagId: { 'project-a': 2 },
        },
      })
      .mockReturnValueOnce(pendingQuery.promise);

    const { result, rerender } = renderHook(
      (params: Parameters<typeof useWorkspaceQuery>[0]) => useWorkspaceQuery(params),
      {
        initialProps: {
          ...baseParams,
          historyItems: items,
          scope: { kind: 'inbox' as const },
        } as Parameters<typeof useWorkspaceQuery>[0],
      },
    );

    await waitFor(() => {
      expect(result.current.itemCounts.untagged).toBe(1);
      expect(result.current.itemCounts.byTagId?.['project-a']).toBe(2);
    });

    // Switch scope to project-a (query is now in-flight via pendingQuery)
    rerender({
      ...baseParams,
      historyItems: items,
      scope: { kind: 'project' as const, projectId: 'project-a' },
    });

    // During loading of the new scope, itemCounts and summary must remain stable (not 0)
    expect(result.current.isInitialLoading).toBe(true);
    expect(result.current.itemCounts.untagged).toBe(1);
    expect(result.current.itemCounts.byTagId?.['project-a']).toBe(2);
    expect(result.current.summary.totalItems).toBe(2);
    expect(result.current.filteredItemCount).toBe(2);

    // Resolve the query
    await act(async () => {
      pendingQuery.resolve({
        ...page([items[1], items[2]], 2, false),
        itemCounts: {
          untagged: 1,
          trash: 0,
          byTagId: { 'project-a': 2 },
        },
      });
    });

    await waitFor(() => {
      expect(result.current.isInitialLoading).toBe(false);
      expect(result.current.filteredItems).toHaveLength(2);
    });
  });
});
