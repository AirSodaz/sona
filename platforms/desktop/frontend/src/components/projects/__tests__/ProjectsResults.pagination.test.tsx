import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ProjectsResults } from '../ProjectsResults';

vi.mock('react-virtuoso', async () => {
  const React = await import('react');
  const VirtualList = ({ components, context, data, endReached, itemContent }: any, ref: unknown) => {
    void ref;
    const Footer = components?.Footer;
    return (
      <div>
        {data.map((item: any, index: number) => itemContent(index, item))}
        <button type="button" onClick={() => endReached?.(data.length - 1)}>Reach end</button>
        {Footer ? <Footer context={context} /> : null}
      </div>
    );
  };
  return {
    Virtuoso: React.forwardRef(VirtualList),
    VirtuosoGrid: React.forwardRef(VirtualList),
  };
});

vi.mock('../../history/HistoryItem', () => ({
  HistoryItem: ({ item }: any) => <div>{item.title}</div>,
}));

const historyItem = {
  id: 'item-1',
  title: 'First item',
  timestamp: 1,
  duration: 1,
  audioPath: 'item.wav',
  transcriptPath: 'item.json',
  previewText: '',
  searchContent: '',
  type: 'recording' as const,
  projectId: null,
};

function renderResults(overrides: Record<string, unknown> = {}) {
  const props = {
    activeSearchResultId: null,
    browseProject: null,
    filteredAndSortedItems: [historyItem],
    filteredItemCount: 1,
    handleOpenItem: vi.fn(),
    isHistoryInteractionLocked: false,
    isAllItemsScope: true,
    isHistoryLoading: false,
    initialLoadError: false,
    isInitialLoading: false,
    isLoadingMore: false,
    isSelectionMode: false,
    loadMoreError: false,
    onDeleteHistoryItem: vi.fn(),
    onLoadMore: vi.fn(),
    onRenameHistoryItem: vi.fn(),
    onRetryInitialLoad: vi.fn(),
    onScroll: vi.fn(),
    onToggleSelection: vi.fn(),
    resetBrowseState: vi.fn(),
    scopeItemCount: 1,
    searchMatchByItemId: new Map(),
    searchQuery: '',
    selectedHistoryId: null,
    selectedIds: [],
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? '',
    viewMode: 'list' as const,
    ...overrides,
  };

  render(<ProjectsResults {...props as any} />);
  return props;
}

it('loads the next workspace page when the virtual list reaches the end', () => {
  const props = renderResults();

  fireEvent.click(screen.getByRole('button', { name: 'Reach end' }));

  expect(props.onLoadMore).toHaveBeenCalledTimes(1);
});

it('shows a retry action instead of an empty state when the first page fails', () => {
  const props = renderResults({
    filteredAndSortedItems: [],
    filteredItemCount: 0,
    initialLoadError: true,
    scopeItemCount: 0,
  });

  expect(screen.queryByText('No items in this workspace yet.')).toBeNull();
  expect(screen.getByText('Workspace items could not be loaded')).toBeDefined();
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

  expect(props.onRetryInitialLoad).toHaveBeenCalledTimes(1);
});

it('keeps loaded results and exposes a retry action after pagination fails', () => {
  const props = renderResults({ loadMoreError: true });

  expect(screen.getByText('First item')).toBeDefined();
  fireEvent.click(screen.getByRole('button', { name: 'Retry loading more items' }));

  expect(props.onLoadMore).toHaveBeenCalledTimes(1);
});
