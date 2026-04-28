import React from 'react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDialogStore } from '../../../stores/dialogStore';
import { useErrorDialogStore } from '../../../stores/errorDialogStore';
import type { HistoryItem } from '../../../types/history';
import type { ProjectRecord } from '../../../types/project';
import { useWorkspaceBrowseState } from '../hooks/useWorkspaceBrowseState';

const projectAlpha: ProjectRecord = {
  id: 'project-1',
  name: 'Alpha',
  description: 'Alpha project',
  icon: '🧪',
  createdAt: 1,
  updatedAt: 1,
  defaults: {
    summaryTemplateId: 'general',
    translationLanguage: 'zh',
    polishPresetId: 'general',
    exportFileNamePrefix: '',
    enabledTextReplacementSetIds: [],
    enabledHotwordSetIds: [],
    enabledPolishKeywordSetIds: [],
    enabledSpeakerProfileIds: [],
  },
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
});
