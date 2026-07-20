import React from 'react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDialogStore } from '../../../stores/dialogStore';
import { useErrorDialogStore } from '../../../stores/errorDialogStore';
import type { HistoryItem } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import { historyQueryWorkspace } from '../../../services/tauri/history';
import { useWorkspaceBrowseState } from '../hooks/useWorkspaceBrowseState';
import { buildWorkspaceViewModel } from '../hooks/workspaceViewModel';

vi.mock('../../../services/tauri/history', () => ({
  historyQueryWorkspace: vi.fn(),
}));

const projectAlpha: ProjectRecord = {
  id: 'project-1',
  name: 'Alpha',
  description: 'Alpha project',
  icon: '🧪',
  createdAt: 1,
  updatedAt: 1,
};

const historyItems: HistoryItem[] = [
  {
    id: 'hist-1',
    title: 'Alpha Plan',
    timestamp: Date.now(),
    duration: 120,
    audioPath: 'audio.wav',
    transcriptPath: 'hist-1.json',
    previewText: 'Roadmap preview',
    searchContent: 'Roadmap preview',
    type: 'recording',
    projectId: 'project-1',
  },
];

describe('useWorkspaceBrowseState', () => {
  const t = (key: string, options?: Record<string, unknown>) => {
    if (typeof options?.defaultValue === 'string') {
      return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_match, variable: string) => String(options?.[variable] ?? ''));
    }
    return key;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(historyQueryWorkspace).mockResolvedValue({
      filteredItems: historyItems,
      searchMatchByItemId: {},
      filteredItemCount: historyItems.length,
      hasMore: false,
      summary: {
        totalItems: historyItems.length,
        totalDuration: historyItems.reduce((total, item) => total + item.duration, 0),
        latestTimestamp: historyItems[0]?.timestamp ?? null,
        recordingCount: historyItems.length,
        batchCount: 0,
      },
      itemCounts: {
        inbox: 0,
        byProjectId: {
          'project-1': historyItems.length,
        },
      },
    });
    useDialogStore.setState({
      ...useDialogStore.getState(),
      isOpen: false,
    });
    useErrorDialogStore.setState({
      ...useErrorDialogStore.getState(),
      isOpen: false,
    });
  });

  it('resets local browse state when the scope changes', async () => {
    const filterMenuRef = { current: document.createElement('div') } as React.RefObject<HTMLDivElement>;
    const searchInputRef = { current: document.createElement('input') } as React.RefObject<HTMLInputElement>;
    const onOpenItem = vi.fn();

    const { result } = renderHook(() => useWorkspaceBrowseState({
      activeProjectId: 'project-1',
      historyItems,
      projects: [projectAlpha],
      filterMenuRef,
      isSelectionMode: false,
      searchInputRef,
      t,
      onOpenItem,
    }));

    await act(async () => {
      result.current.setSearchQuery('roadmap');
      result.current.setFilterType('batch');
      result.current.setDateFilter('today');
      result.current.setIsFilterMenuOpen(true);
      result.current.setBrowseScope('all');
    });

    await waitFor(() => {
      expect(result.current.searchQuery).toBe('');
      expect(result.current.filterType).toBe('all');
      expect(result.current.dateFilter).toBe('all');
      expect(result.current.isFilterMenuOpen).toBe(false);
    });
  });

  it('clears the active search result when selection mode becomes active', async () => {
    const filterMenuRef = { current: document.createElement('div') } as React.RefObject<HTMLDivElement>;
    const searchInputRef = { current: document.createElement('input') } as React.RefObject<HTMLInputElement>;
    const onOpenItem = vi.fn();

    const { result, rerender } = renderHook(
      ({ isSelectionMode }) => useWorkspaceBrowseState({
        activeProjectId: 'project-1',
        historyItems,
        projects: [projectAlpha],
        filterMenuRef,
        isSelectionMode,
        searchInputRef,
        t,
        onOpenItem,
      }),
      {
        initialProps: { isSelectionMode: false },
      },
    );

    await act(async () => {
      result.current.setSearchQuery('roadmap');
      result.current.setActiveSearchResultId('hist-1');
    });

    rerender({ isSelectionMode: true });

    await waitFor(() => {
      expect(result.current.activeSearchResultId).toBeNull();
    });
  });

  it('ignores stale workspace query results from older async requests', async () => {
    const filterMenuRef = { current: document.createElement('div') } as React.RefObject<HTMLDivElement>;
    const searchInputRef = { current: document.createElement('input') } as React.RefObject<HTMLInputElement>;
    const onOpenItem = vi.fn();
    let resolveFirst: ((value: Awaited<ReturnType<typeof historyQueryWorkspace>>) => void) | null = null;

    const firstItem: HistoryItem = {
      ...historyItems[0],
      id: 'hist-first',
      title: 'First stale result',
    };
    const secondItem: HistoryItem = {
      ...historyItems[0],
      id: 'hist-second',
      title: 'Second current result',
    };

    vi.mocked(historyQueryWorkspace)
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveFirst = resolve;
      }))
      .mockResolvedValueOnce({
        filteredItems: [secondItem],
        searchMatchByItemId: {},
        filteredItemCount: 1,
        hasMore: false,
        summary: {
          totalItems: 1,
          totalDuration: secondItem.duration,
          latestTimestamp: secondItem.timestamp,
          recordingCount: 1,
          batchCount: 0,
        },
        itemCounts: {
          inbox: 0,
          byProjectId: {
            'project-1': 1,
          },
        },
      });

    const { result } = renderHook(() => useWorkspaceBrowseState({
      activeProjectId: 'project-1',
      historyItems,
      projects: [projectAlpha],
      filterMenuRef,
      isSelectionMode: false,
      searchInputRef,
      t,
      onOpenItem,
    }));

    await act(async () => {
      result.current.setSearchQuery('second');
    });

    await waitFor(() => {
      expect(result.current.filteredAndSortedItems.map((item) => item.id)).toEqual(['hist-second']);
    });

    await act(async () => {
      resolveFirst?.({
        filteredItems: [firstItem],
        searchMatchByItemId: {},
        filteredItemCount: 1,
        hasMore: false,
        summary: {
          totalItems: 1,
          totalDuration: firstItem.duration,
          latestTimestamp: firstItem.timestamp,
          recordingCount: 1,
          batchCount: 0,
        },
        itemCounts: {
          inbox: 0,
          byProjectId: {
            'project-1': 1,
          },
        },
      });
    });

    await waitFor(() => {
      expect(result.current.filteredAndSortedItems.map((item) => item.id)).toEqual(['hist-second']);
    });
  });

  it('derives browse copy, filters, summary chips, and counts from the query result', () => {
    const viewModel = buildWorkspaceViewModel({
      browseProject: projectAlpha,
      browseScope: projectAlpha.id,
      dateFilter: 'today',
      projects: [projectAlpha],
      filterType: 'batch',
      queryResult: {
        filteredItems: historyItems,
        searchMatchByItemId: {},
        filteredItemCount: historyItems.length,
        hasMore: false,
        summary: {
          totalItems: historyItems.length,
          totalDuration: 3600,
          latestTimestamp: historyItems[0].timestamp,
          recordingCount: 0,
          batchCount: 1,
        },
        itemCounts: {
          inbox: 2,
          byProjectId: {
            [projectAlpha.id]: historyItems.length,
          },
        },
      },
      t,
    });

    expect(viewModel.headerTitle).toBe(projectAlpha.name);
    expect(viewModel.headerDescription).toBe(projectAlpha.description);
    expect(viewModel.searchInputLabel).toBe('Search in Alpha...');
    expect(viewModel.activeFilterCount).toBe(2);
    expect(viewModel.filterPopoverHint).toBe('Batch imports · Today');
    expect(viewModel.itemCounts.get(null)).toBe(2);
    expect(viewModel.itemCounts.get(projectAlpha.id)).toBe(1);
    expect(viewModel.summaryChips.map((chip) => chip.value)).toEqual([
      '1',
      '1h 0m',
      expect.any(String),
      '0 recordings / 1 imports',
    ]);
  });

  it('falls back to the empty workspace result when the query fails', async () => {
    const filterMenuRef = { current: document.createElement('div') } as React.RefObject<HTMLDivElement>;
    const searchInputRef = { current: document.createElement('input') } as React.RefObject<HTMLInputElement>;
    const onOpenItem = vi.fn();

    vi.mocked(historyQueryWorkspace)
      .mockResolvedValueOnce({
        filteredItems: historyItems,
        searchMatchByItemId: {},
        filteredItemCount: historyItems.length,
        hasMore: false,
        summary: {
          totalItems: historyItems.length,
          totalDuration: historyItems[0].duration,
          latestTimestamp: historyItems[0].timestamp,
          recordingCount: 1,
          batchCount: 0,
        },
        itemCounts: {
          inbox: 0,
          byProjectId: {
            [projectAlpha.id]: historyItems.length,
          },
        },
      })
      .mockRejectedValueOnce(new Error('query failed'));

    const { result } = renderHook(() => useWorkspaceBrowseState({
      activeProjectId: 'project-1',
      historyItems,
      projects: [projectAlpha],
      filterMenuRef,
      isSelectionMode: false,
      searchInputRef,
      t,
      onOpenItem,
    }));

    await waitFor(() => {
      expect(result.current.filteredAndSortedItems).toHaveLength(1);
    });

    await act(async () => {
      result.current.setSearchQuery('missing');
      result.current.setActiveSearchResultId('hist-1');
    });

    await waitFor(() => {
      expect(result.current.filteredAndSortedItems).toEqual([]);
      expect(result.current.scopeItemCount).toBe(0);
      expect(result.current.activeSearchResultId).toBeNull();
      expect(result.current.itemCounts.get(null)).toBe(0);
    });
  });

  it('does not focus workspace search from Ctrl+F while focus belongs to detail or modal surfaces', async () => {
    const filterMenuRef = { current: document.createElement('div') } as React.RefObject<HTMLDivElement>;
    const searchInput = document.createElement('input');
    const searchInputRef = { current: searchInput } as React.RefObject<HTMLInputElement>;
    const detailPane = document.createElement('div');
    const detailInput = document.createElement('input');
    const settingsOverlay = document.createElement('div');
    const settingsInput = document.createElement('input');
    const onOpenItem = vi.fn();

    detailPane.className = 'projects-detail-pane';
    detailPane.appendChild(detailInput);
    settingsOverlay.className = 'settings-overlay';
    settingsOverlay.appendChild(settingsInput);
    document.body.append(detailPane, settingsOverlay);
    document.body.appendChild(searchInput);

    try {
      renderHook(() => useWorkspaceBrowseState({
        activeProjectId: 'project-1',
        historyItems,
        projects: [projectAlpha],
        filterMenuRef,
        isSelectionMode: false,
        searchInputRef,
        t,
        onOpenItem,
      }));

      detailInput.focus();
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true }));
      expect(document.activeElement).toBe(detailInput);

      settingsInput.focus();
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true }));
      expect(document.activeElement).toBe(settingsInput);
    } finally {
      detailPane.remove();
      settingsOverlay.remove();
      searchInput.remove();
    }
  });
});
