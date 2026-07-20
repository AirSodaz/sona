import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactElement } from 'react';
import {
  act,
  fireEvent,
  render as testingLibraryRender,
  screen,
  waitFor,
} from '@testing-library/react';
import { ProjectsView } from '../ProjectsView';
import { ContextMenuProvider } from '../context-menu/ContextMenuProvider';
import { ProjectsResults } from '../projects/ProjectsResults';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

window.HTMLElement.prototype.scrollIntoView = vi.fn();

const render = (ui: ReactElement) => testingLibraryRender(
  <ContextMenuProvider>{ui}</ContextMenuProvider>,
);
const virtuosoScrollToIndexMock = vi.hoisted(() => vi.fn());
const virtuosoGridScrollToIndexMock = vi.hoisted(() => vi.fn());
const workspaceQueryBackendMock = vi.hoisted(() => ({
  impl: null as null | ((args: any, items: any[]) => any),
}));
const aiRenameModuleState = vi.hoisted(() => ({
  loadCount: 0,
  generateAiTitleForHistoryItem: vi.fn().mockResolvedValue('AI Project Title'),
}));

vi.mock('../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn().mockResolvedValue([]),
    getActiveProjectId: vi.fn().mockResolvedValue(null),
    create: vi.fn().mockImplementation(async ({ name, description, icon }: any) => ({
      id: 'project-new',
      name,
      description,
      icon: icon ?? '',
      createdAt: 1,
      updatedAt: 1,
    })),
    update: vi.fn().mockImplementation(async (id: string, updates: any) => ({
      id,
      name: updates.name || 'Alpha',
      description: updates.description || '',
      icon: updates.icon ?? '🧪',
      createdAt: 1,
      updatedAt: 2,
    })),
    delete: vi.fn().mockResolvedValue(undefined),
    setActiveProjectId: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../services/historyService', () => ({
  historyService: {
    getAll: vi.fn().mockResolvedValue([]),
    loadTranscript: vi.fn().mockResolvedValue([]),
    getAudioUrl: vi.fn().mockResolvedValue('asset:///audio.wav'),
    updateTranscript: vi.fn().mockResolvedValue(undefined),
    updateItemMeta: vi.fn().mockResolvedValue(undefined),
    updateProjectAssignments: vi.fn().mockResolvedValue(undefined),
    updateTagAssignments: vi.fn().mockResolvedValue(undefined),
    deleteRecording: vi.fn().mockResolvedValue(undefined),
    deleteRecordings: vi.fn().mockResolvedValue(undefined),
    restoreRecordings: vi.fn().mockResolvedValue(undefined),
    purgeRecordings: vi.fn().mockResolvedValue(undefined),
    openHistoryFolder: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../services/aiRenameService', () => {
  aiRenameModuleState.loadCount += 1;
  return {
    generateAiTitleForHistoryItem: aiRenameModuleState.generateAiTitleForHistoryItem,
  };
});

vi.mock('../../services/tauri/history', () => {
  const summarize = (items: any[]) => ({
    totalItems: items.length,
    totalDuration: items.reduce((total, item) => total + Number(item.duration || 0), 0),
    latestTimestamp: items.reduce<number | null>((latest, item) => {
      const timestamp = Number(item.timestamp || 0);
      return latest === null || timestamp > latest ? timestamp : latest;
    }, null),
    recordingCount: items.filter((item) => item.type !== 'batch').length,
    batchCount: items.filter((item) => item.type === 'batch').length,
  });

  return {
    historyQueryWorkspace: vi.fn(async (args: any) => {
      const { scope } = args;
      const { useHistoryStore } = await import('../../stores/historyStore');
      const items = useHistoryStore.getState().items;
      if (workspaceQueryBackendMock.impl) {
        return workspaceQueryBackendMock.impl(args, items);
      }

      const scopedItems = items.filter((item: any) => {
        const tagIds = item.tagIds ?? (item.projectId ? [item.projectId] : []);
        const isDeleted = item.deletedAt != null;
        if (scope.kind === 'all') {
          return !isDeleted;
        }
        if (scope.kind === 'untagged') {
          return !isDeleted && tagIds.length === 0;
        }
        if (scope.kind === 'trash') {
          return isDeleted;
        }
        return !isDeleted && tagIds.includes(scope.tagId);
      });
      const byTagId: Record<string, number> = {};
      let untagged = 0;
      let trash = 0;
      items.forEach((item: any) => {
        if (item.deletedAt != null) {
          trash += 1;
          return;
        }
        const tagIds = item.tagIds ?? (item.projectId ? [item.projectId] : []);
        if (tagIds.length > 0) {
          tagIds.forEach((tagId: string) => {
            byTagId[tagId] = (byTagId[tagId] || 0) + 1;
          });
        } else {
          untagged += 1;
        }
      });

      return {
        filteredItems: scopedItems,
        searchMatchByItemId: Object.fromEntries(scopedItems.map((item: any) => [item.id, null])),
        filteredItemCount: scopedItems.length,
        hasMore: false,
        summary: summarize(scopedItems),
        itemCounts: {
          untagged,
          trash,
          byTagId,
        },
      };
    }),
  };
});

vi.mock('../TranscriptWorkbench', () => ({
  TranscriptWorkbench: ({ title, onClose }: any) => (
    <div>
      <div>TranscriptEditor</div>
      <button>Detail Focus</button>
      <button onClick={onClose}>Close</button>
      {title && <div>{title}</div>}
    </div>
  ),
}));

vi.mock('../TranscriptEditor', () => ({
  TranscriptEditor: () => <div>TranscriptEditor</div>,
}));

vi.mock('../AudioPlayer', () => ({
  AudioPlayer: () => <div>AudioPlayer</div>,
}));

vi.mock('../ErrorBoundary', () => ({
  ErrorBoundary: ({ children }: any) => <div>{children}</div>,
}));

vi.mock('../history/HistoryItem', () => ({
  HistoryItem: ({
    item,
    onLoad,
    isSelectionMode,
    isSelected,
    isKeyboardActive,
    onToggleSelection,
    onRename,
    searchQuery,
    searchSnippet,
    showProjectBadge = true,
    onOpenContextMenu,
    isContextMenuOpen,
    isLoadDisabled,
    isRenameDisabled,
    isDeleteDisabled,
  }: any) => (
    <div
      id={`workspace-search-result-${item.id}`}
      data-testid={`history-item-${item.id}`}
      data-search-query={searchQuery || ''}
      data-keyboard-active={isKeyboardActive ? 'true' : 'false'}
      data-context-menu-open={isContextMenuOpen ? 'true' : 'false'}
      data-load-disabled={isLoadDisabled ? 'true' : 'false'}
      data-rename-disabled={isRenameDisabled ? 'true' : 'false'}
      data-delete-disabled={isDeleteDisabled ? 'true' : 'false'}
      onContextMenu={(event) => {
        event.preventDefault();
        if (isSelectionMode) {
          return;
        }
        onOpenContextMenu?.(item.id, {
          anchor: event.currentTarget,
          point: { x: event.clientX, y: event.clientY },
          invocation: 'pointer',
        });
      }}
    >
      <button onClick={() => onLoad(item)}>{item.title}</button>
      {showProjectBadge && <span>{`Project ${item.projectId ?? 'inbox'}`}</span>}
      {searchQuery && <span>{`Query ${searchQuery}`}</span>}
      {searchSnippet?.text && <span>{`Snippet ${searchSnippet.text}`}</span>}
      {isSelected && <span>{`Active ${item.id}`}</span>}
      {onRename && !isSelectionMode && (
        <button onClick={() => onRename(item.id)}>
          {`Rename ${item.title}`}
        </button>
      )}
      {isSelectionMode && (
        <button onClick={() => onToggleSelection?.(item.id)}>
          {isSelected ? `Selected ${item.id}` : `Select ${item.id}`}
        </button>
      )}
    </div>
  ),
}));

vi.mock('react-virtuoso', async () => {
  const React = await import('react');
  const renderLimit = 20;
  const Virtuoso = React.forwardRef(({
    className,
    components,
    context,
    data = [],
    itemContent,
    onScroll,
  }: any, ref) => {
    React.useImperativeHandle(ref, () => ({
      scrollToIndex: virtuosoScrollToIndexMock,
      scrollTo: vi.fn(),
      scrollBy: vi.fn(),
      getState: vi.fn(),
    }));

    const Header = components?.Header;
    const Footer = components?.Footer;
    const List = components?.List;
    const items = data.slice(0, renderLimit).map((item: any, index: number) => itemContent(index, item, context));
    return (
      <div className={className} onScroll={onScroll} data-testid="projects-virtuoso-list">
        {Header && <Header context={context} />}
        {List ? <List context={context}>{items}</List> : items}
        {Footer && <Footer context={context} />}
      </div>
    );
  });
  const VirtuosoGrid = React.forwardRef(({
    className,
    components,
    data = [],
    itemContent,
    listClassName,
    onScroll,
  }: any, ref) => {
    React.useImperativeHandle(ref, () => ({
      scrollToIndex: virtuosoGridScrollToIndexMock,
      scrollTo: vi.fn(),
      scrollBy: vi.fn(),
    }));

    const Header = components?.Header;
    const Footer = components?.Footer;
    return (
      <div className={className} onScroll={onScroll} data-testid="projects-virtuoso-grid">
        {Header && <Header />}
        <div className={listClassName}>
          {data.slice(0, renderLimit).map((item: any, index: number) => itemContent(index, item))}
        </div>
        {Footer && <Footer />}
      </div>
    );
  });

  return { Virtuoso, VirtuosoGrid };
});

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: any) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) => String(options?.[variable] ?? ''));
      }
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('ProjectsView', () => {
  const getButtonByContent = (label: string) => {
    const button = screen
      .getAllByRole('button')
      .find((candidate) => candidate.tagName === 'BUTTON' && candidate.textContent?.includes(label));
    expect(button).not.toBeNull();
    return button as HTMLButtonElement;
  };

  const getRailItemIcon = (button: HTMLElement) => button.querySelector('.projects-rail-item-icon');

  const getMainTitleIcon = () => document.querySelector('.projects-main-title-icon');

  const getDetailPlaceholder = () => document.querySelector('.projects-detail-pane[data-projects-detail-placeholder="true"]');

  const selectDropdownOption = (ariaLabel: string, optionLabel: string) => {
    fireEvent.click(screen.getByRole('button', { name: ariaLabel }));
    fireEvent.click(screen.getByRole('option', { name: optionLabel }));
  };

  const openFilterMenu = () => {
    if (screen.queryByRole('dialog', { name: 'Filter' })) {
      return;
    }

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }));
  };

  const waitForInitialHistoryLoad = async () => {
    const { historyService } = await import('../../services/historyService');

    await waitFor(() => {
      expect(historyService.getAll).toHaveBeenCalled();
      expect(useHistoryStore.getState().isLoading).toBe(false);
    });
  };

  const clickAsync = async (element: HTMLElement) => {
    await act(async () => {
      fireEvent.click(element);
    });
  };

  const createHistoryItems = (count: number, projectId: string | null = null) => (
    Array.from({ length: count }, (_, index) => ({
      id: `hist-${index}`,
      title: `History ${index}`,
      timestamp: Date.now() - index,
      duration: 12 + index,
      audioPath: `audio-${index}.wav`,
      transcriptPath: `hist-${index}.json`,
      previewText: `Preview ${index}`,
      type: 'recording',
      projectId,
    }))
  );

  const summarizeWorkspaceItems = (items: any[]) => ({
    totalItems: items.length,
    totalDuration: items.reduce((total, item) => total + Number(item.duration || 0), 0),
    latestTimestamp: items.reduce<number | null>((latest, item) => {
      const timestamp = Number(item.timestamp || 0);
      return latest === null || timestamp > latest ? timestamp : latest;
    }, null),
    recordingCount: items.filter((item) => item.type !== 'batch').length,
    batchCount: items.filter((item) => item.type === 'batch').length,
  });

  const buildWorkspaceQueryResult = ({
    filteredItems,
    scopedItems = filteredItems,
    searchMatchByItemId = {},
    allItems = scopedItems,
  }: {
    filteredItems: any[];
    scopedItems?: any[];
    searchMatchByItemId?: Record<string, any>;
    allItems?: any[];
  }) => {
    const byTagId: Record<string, number> = {};
    let untagged = 0;
    let trash = 0;
    allItems.forEach((item) => {
      if (item.deletedAt != null) {
        trash += 1;
        return;
      }
      const tagIds = item.tagIds ?? (item.projectId ? [item.projectId] : []);
      if (tagIds.length > 0) {
        tagIds.forEach((tagId: string) => {
          byTagId[tagId] = (byTagId[tagId] || 0) + 1;
        });
      } else {
        untagged += 1;
      }
    });

    return {
      filteredItems,
      searchMatchByItemId: Object.fromEntries(
        filteredItems.map((item) => [item.id, searchMatchByItemId[item.id] ?? null]),
      ),
      filteredItemCount: filteredItems.length,
      hasMore: false,
      summary: summarizeWorkspaceItems(scopedItems),
      itemCounts: {
        untagged,
        trash,
        byTagId,
      },
    };
  };

  const makeSearchMatch = (text: string, field = 'previewText') => ({
    matchedField: field,
    titleMatch: field === 'title' ? { start: 0, end: text.length } : null,
    displaySnippet: {
      text,
      highlightStart: 0,
      highlightEnd: text.length,
    },
  });

  beforeEach(async () => {
    vi.clearAllMocks();
    workspaceQueryBackendMock.impl = null;

    useProjectStore.setState({
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: 'Project alpha',
          icon: '🧪',
          createdAt: 1,
          updatedAt: 1,
        },
      ],
      activeProjectId: null,
      isLoading: false,
      error: null,
    });

    useHistoryStore.setState({
      items: [
        {
          id: 'hist-inbox',
          title: 'Inbox Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-inbox.json',
          previewText: 'Preview',
          projectId: null,
        },
      ],
      isLoading: false,
      error: null,
    } as any);

    const { historyService } = await import('../../services/historyService');
    (historyService.getAll as any).mockImplementation(async () => useHistoryStore.getState().items);

    useTranscriptStore.setState({
      mode: 'projects',
      sourceHistoryId: null,
      audioUrl: null,
      segments: [],
    });

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        translationLanguage: 'en',
        polishPresetId: 'lecture',
        polishCustomPresets: [],
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand Terms', enabled: true, keywords: 'Sona' },
          { id: 'kw-2', name: 'Style Guide', enabled: false, keywords: 'Sentence case' },
        ],
      },
    });

    useDialogStore.setState({
      ...useDialogStore.getState(),
      confirm: vi.fn().mockResolvedValue(true),
      showError: vi.fn().mockResolvedValue(undefined),
    });
  });

  it('renders All Items and Inbox in the rail while keeping a single New Project CTA', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(getButtonByContent('All Items')).toBeDefined();
    expect(getButtonByContent('Untagged')).toBeDefined();
    expect(screen.getAllByRole('button', { name: 'New Tag' })).toHaveLength(1);
  });

  it('loads the AI rename service only when the AI rename action is used', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(aiRenameModuleState.loadCount).toBe(0);

    fireEvent.click(screen.getByRole('button', { name: 'Rename Inbox Item' }));
    expect(aiRenameModuleState.loadCount).toBe(0);

    fireEvent.click(screen.getByRole('button', { name: 'AI Auto-rename' }));

    await waitFor(() => {
      expect(aiRenameModuleState.generateAiTitleForHistoryItem).toHaveBeenCalledWith('hist-inbox');
    });
    expect(aiRenameModuleState.loadCount).toBe(1);
    await waitFor(() => {
      expect(screen.getByDisplayValue('AI Project Title')).toBeDefined();
    });
  });

  it('opens the history context menu and reuses the existing rename flow', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const historyItem = screen.getByTestId('history-item-hist-inbox');
    fireEvent.contextMenu(historyItem, { clientX: 160, clientY: 220 });

    expect(screen.getByRole('menu', { name: 'Actions for Inbox Item' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Open' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Rename' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Delete' })).toBeDefined();
    expect(historyItem.dataset.contextMenuOpen).toBe('true');
    expect(useTranscriptStore.getState().sourceHistoryId).toBeNull();

    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }));

    expect(await screen.findByDisplayValue('Inbox Item')).toBeDefined();
    expect(screen.queryByRole('menu')).toBeNull();
    expect(useTranscriptStore.getState().sourceHistoryId).toBeNull();
  });

  it('does not submit a rename after the target is removed', async () => {
    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 160,
      clientY: 220,
    });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }));
    expect(await screen.findByDisplayValue('Inbox Item')).toBeDefined();

    act(() => {
      useHistoryStore.setState({ items: [] });
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(historyService.updateItemMeta).not.toHaveBeenCalled();
  });

  it('does not submit a rename after the target becomes the active live draft', async () => {
    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 160,
      clientY: 220,
    });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }));
    expect(await screen.findByDisplayValue('Inbox Item')).toBeDefined();

    act(() => {
      const currentItem = useHistoryStore.getState().items[0];
      useHistoryStore.setState({
        items: [{
          ...currentItem,
          status: 'draft',
          draftSource: 'live_record',
        }],
      } as any);
      useTranscriptStore.setState({
        sourceHistoryId: 'hist-inbox',
        isRecording: true,
      });
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(historyService.updateItemMeta).not.toHaveBeenCalled();
  });

  it('opens project settings for a project selected from the rail context menu', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const projectButton = getButtonByContent('Alpha');
    fireEvent.contextMenu(projectButton, { clientX: 92, clientY: 148 });

    expect(screen.getByRole('menu', { name: 'Actions for Alpha' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Open' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Tag Settings' })).toBeDefined();

    fireEvent.click(screen.getByRole('menuitem', { name: 'Tag Settings' }));

    await waitFor(() => {
      expect(projectService.setActiveProjectId).toHaveBeenCalledWith('project-1');
      expect(screen.getByText('Tag settings')).toBeDefined();
    });
  });

  it('closes an open workspace menu when its target is removed', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    expect(screen.getByRole('menu', { name: 'Actions for Inbox Item' })).toBeDefined();

    act(() => {
      useHistoryStore.setState({ items: [] });
    });

    await waitFor(() => {
      expect(screen.queryByRole('menu')).toBeNull();
    });
  });

  it('closes an open workspace menu when the target metadata changes', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    expect(screen.getByRole('menu', { name: 'Actions for Inbox Item' })).toBeDefined();

    act(() => {
      const currentItem = useHistoryStore.getState().items[0];
      useHistoryStore.setState({
        items: [{ ...currentItem, title: 'Updated Inbox Item' }],
      } as any);
    });

    await waitFor(() => {
      expect(screen.queryByRole('menu')).toBeNull();
    });
  });

  it('resolves the latest history item when a context action runs', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    const renameAction = screen.getByRole('menuitem', { name: 'Rename' });

    act(() => {
      const currentItem = useHistoryStore.getState().items[0];
      useHistoryStore.setState({
        items: [{ ...currentItem, title: 'Latest Inbox Item' }],
      } as any);
      fireEvent.click(renameAction);
    });

    expect(await screen.findByDisplayValue('Latest Inbox Item')).toBeDefined();
  });

  it('rechecks the live draft lock when a context action runs', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    const deleteAction = screen.getByRole('menuitem', { name: 'Delete' });

    act(() => {
      const currentItem = useHistoryStore.getState().items[0];
      useHistoryStore.setState({
        items: [{
          ...currentItem,
          status: 'draft',
          draftSource: 'live_record',
        }],
      } as any);
      useTranscriptStore.setState({
        sourceHistoryId: 'hist-inbox',
        isRecording: true,
      });
      fireEvent.click(deleteAction);
    });

    expect(useDialogStore.getState().confirm).not.toHaveBeenCalled();
  });

  it('rechecks the live draft lock before a project context action switches scope', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(getButtonByContent('Alpha'), { clientX: 80, clientY: 120 });
    const openAction = screen.getByRole('menuitem', { name: 'Open' });

    act(() => {
      useHistoryStore.setState({
        items: [{
          id: 'hist-live',
          title: 'Live Draft',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'live.wav',
          transcriptPath: 'hist-live.json',
          previewText: 'Live preview',
          type: 'recording',
          projectId: 'project-1',
          status: 'draft',
          draftSource: 'live_record',
        }],
      } as any);
      useTranscriptStore.setState({
        sourceHistoryId: 'hist-live',
        isRecording: true,
      });
      fireEvent.click(openAction);
    });

    expect(projectService.setActiveProjectId).not.toHaveBeenCalled();
    expect(useProjectStore.getState().activeProjectId).toBeNull();
  });

  it('opens and deletes history items through the context menu actions', async () => {
    const { historyService } = await import('../../services/historyService');

    const firstRender = render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Open' }));

    await waitFor(() => {
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-inbox');
      expect(getDetailPlaceholder()).not.toBeNull();
    });

    firstRender.unmount();
    useTranscriptStore.setState({ sourceHistoryId: null, segments: [], audioUrl: null });
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    await clickAsync(screen.getByRole('menuitem', { name: 'Delete' }));

    await waitFor(() => {
      expect(historyService.deleteRecording).toHaveBeenCalledWith('hist-inbox');
    });
  });

  it('disables opening the current project while keeping project settings available', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const projectButton = getButtonByContent('Alpha');
    vi.spyOn(projectButton, 'getBoundingClientRect').mockReturnValue({
      x: 24,
      y: 72,
      width: 280,
      height: 64,
      top: 72,
      right: 304,
      bottom: 136,
      left: 24,
      toJSON: () => ({}),
    });
    fireEvent.keyDown(projectButton, { key: 'F10', shiftKey: true });

    expect((screen.getByRole('menuitem', { name: 'Open' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('menuitem', { name: 'Tag Settings' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('disables other project actions during an active live draft', async () => {
    useProjectStore.setState({ activeProjectId: null });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-live',
          title: 'Live Draft',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'live.wav',
          transcriptPath: 'hist-live.json',
          previewText: 'Live preview',
          type: 'recording',
          projectId: 'project-1',
          status: 'draft',
          draftSource: 'live_record',
        },
      ],
    } as any);
    useTranscriptStore.setState({
      sourceHistoryId: 'hist-live',
      isRecording: true,
      segments: [{ id: 'seg-live', start: 0, end: 1, text: 'Live', isFinal: true }],
    });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(getButtonByContent('Alpha'), { clientX: 80, clientY: 120 });
    expect((screen.getByRole('menuitem', { name: 'Open' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('menuitem', { name: 'Tag Settings' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('locks only the current live draft actions when older live drafts remain', async () => {
    const liveDrafts = [
        {
          id: 'hist-old-draft',
          title: 'Old Live Draft',
          timestamp: Date.now() - 1000,
          duration: 8,
          audioPath: 'old-live.wav',
          transcriptPath: 'hist-old-draft.json',
          previewText: 'Old draft',
          type: 'recording',
          projectId: null,
          status: 'draft',
          draftSource: 'live_record',
        },
        {
          id: 'hist-current-draft',
          title: 'Current Live Draft',
          timestamp: Date.now(),
          duration: 4,
          audioPath: 'current-live.wav',
          transcriptPath: 'hist-current-draft.json',
          previewText: 'Current draft',
          type: 'recording',
          projectId: null,
          status: 'draft',
          draftSource: 'live_record',
        },
      ] as any;

    render(
      <ProjectsResults
        activeContextId={null}
        activeSearchResultId={null}
        browseProject={null}
        filteredAndSortedItems={liveDrafts}
        filteredItemCount={liveDrafts.length}
        handleOpenItem={vi.fn()}
        initialLoadError={false}
        isAllItemsScope
        isHistoryLoading={false}
        isInitialLoading={false}
        isLoadingMore={false}
        isSelectionMode={false}
        loadMoreError={false}
        lockedHistoryId="hist-current-draft"
        onDeleteHistoryItem={vi.fn()}
        onLoadMore={vi.fn()}
        onOpenHistoryContextMenu={vi.fn()}
        onRenameHistoryItem={vi.fn()}
        onRetryInitialLoad={vi.fn()}
        onToggleSelection={vi.fn()}
        resetBrowseState={vi.fn()}
        scopeItemCount={liveDrafts.length}
        searchMatchByItemId={new Map()}
        searchQuery=""
        selectedHistoryId={null}
        selectedIds={[]}
        t={(_key, options) => String(options?.defaultValue ?? '')}
        viewMode="list"
      />,
    );

    const oldDraft = screen.getByTestId('history-item-hist-old-draft');
    const currentDraft = screen.getByTestId('history-item-hist-current-draft');
    expect(oldDraft.dataset.loadDisabled).toBe('true');
    expect(oldDraft.dataset.renameDisabled).toBe('false');
    expect(oldDraft.dataset.deleteDisabled).toBe('false');
    expect(currentDraft.dataset.loadDisabled).toBe('false');
    expect(currentDraft.dataset.renameDisabled).toBe('true');
    expect(currentDraft.dataset.deleteDisabled).toBe('true');
  });

  it('closes an open workspace menu when the virtual results viewport scrolls', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-inbox'), {
      clientX: 120,
      clientY: 180,
    });
    expect(screen.getByRole('menu', { name: 'Actions for Inbox Item' })).toBeDefined();

    fireEvent.scroll(screen.getByTestId('projects-virtuoso-list'));

    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('does not render hidden workspace detail or result DOM while inactive', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: createHistoryItems(30, 'project-1'),
    } as any);
    useTranscriptStore.setState({
      sourceHistoryId: 'hist-0',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'Hidden detail', isFinal: true }],
      audioUrl: 'asset:///audio.wav',
    });

    render(<ProjectsView isActive={false} />);
    await waitForInitialHistoryLoad();

    expect(document.querySelector('.projects-workbench[data-projects-inactive="true"]')).not.toBeNull();
    expect(screen.queryByText('TranscriptEditor')).toBeNull();
    expect(screen.queryByTestId('history-item-hist-0')).toBeNull();
  });

  it('windows large workspace result sets instead of rendering every history item', async () => {
    useHistoryStore.setState({
      items: createHistoryItems(150, null),
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(screen.getByTestId('history-item-hist-0')).toBeDefined();
    expect(screen.queryByTestId('history-item-hist-149')).toBeNull();
    expect(screen.getAllByTestId(/history-item-/)).toHaveLength(20);
  });

  it('keeps list and grid gutters while letting table start flush with its header', async () => {
    useHistoryStore.setState({
      items: createHistoryItems(3, null),
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const list = screen.getByTestId('projects-virtuoso-list');
    expect(list.classList.contains('projects-results-scroll')).toBe(true);
    expect(list.classList.contains('projects-results-scroll--list')).toBe(true);
    expect(list.classList.contains('projects-main-scroll--virtual')).toBe(true);
    expect(list.querySelector('.projects-layout-list')?.classList.contains('projects-layout-guttered')).toBe(true);
    expect(list.querySelector('.projects-virtual-spacer--top')).not.toBeNull();
    expect(list.querySelector('.projects-virtual-spacer--bottom')).not.toBeNull();

    await clickAsync(screen.getByRole('button', { name: 'Grid View' }));
    await waitFor(() => {
      expect(screen.getByTestId('projects-virtuoso-grid')).toBeDefined();
    });

    const grid = screen.getByTestId('projects-virtuoso-grid');
    expect(grid.classList.contains('projects-results-scroll')).toBe(true);
    expect(grid.classList.contains('projects-results-scroll--grid')).toBe(true);
    expect(grid.querySelector('.projects-layout-grid')?.classList.contains('projects-layout-guttered')).toBe(true);
    expect(grid.querySelector('.projects-virtual-spacer--top')).not.toBeNull();
    expect(grid.querySelector('.projects-virtual-spacer--bottom')).not.toBeNull();

    await clickAsync(screen.getByRole('button', { name: 'Table View' }));
    await waitFor(() => {
      expect(screen.getByRole('columnheader', { name: 'Name' })).toBeDefined();
    });

    const table = screen.getByTestId('projects-virtuoso-list');
    expect(table.classList.contains('projects-results-scroll')).toBe(true);
    expect(table.classList.contains('projects-results-scroll--table')).toBe(true);
    expect(table.querySelector('.projects-layout-table')?.classList.contains('projects-layout-guttered')).toBe(false);
    expect(table.querySelector('.projects-virtual-spacer--top')).toBeNull();
    expect(table.querySelector('.projects-virtual-spacer--bottom')).toBeNull();
    expect(table.querySelector('.projects-table-header-title')).not.toBeNull();
    expect(table.querySelector('.projects-table-header-meta')).not.toBeNull();
    expect(table.querySelector('.projects-table-header-actions')).not.toBeNull();
    expect(screen.getByRole('columnheader', { name: 'Actions' })).toBeDefined();
  });

  it('hides repeated project badges outside the All Items scope', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-project',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 180,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-project.json',
          previewText: 'Project preview',
          type: 'recording',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(screen.getByText('Project Item')).toBeDefined();
    expect(screen.queryByText('Project project-1')).toBeNull();

    await clickAsync(getButtonByContent('All Items'));

    await waitFor(() => {
      expect(screen.getByText('Project project-1')).toBeDefined();
    });
  });

  it('scrolls the virtualized workspace list to keyboard search results outside the mounted window', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: createHistoryItems(60, 'project-1').map((item) => ({
        ...item,
        searchContent: 'Roadmap search hit',
      })),
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const input = screen.getByRole('textbox', { name: 'Search in Alpha...' });
    fireEvent.change(input, { target: { value: 'roadmap' } });
    await waitFor(() => {
      expect(screen.getByTestId('history-item-hist-0')).toBeDefined();
    });
    for (let index = 0; index < 25; index += 1) {
      fireEvent.keyDown(input, { key: 'ArrowDown' });
    }

    expect(virtuosoScrollToIndexMock).toHaveBeenLastCalledWith(expect.objectContaining({
      align: 'center',
      index: 24,
    }));
  });

  it('renders scope icons in the rail and main header consistently', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const allItemsButton = getButtonByContent('All Items');
    const inboxButton = getButtonByContent('Untagged');
    const projectButton = getButtonByContent('Alpha');

    expect(getRailItemIcon(allItemsButton)).not.toBeNull();
    expect(getRailItemIcon(inboxButton)).not.toBeNull();
    expect(getRailItemIcon(projectButton)).not.toBeNull();
    expect(getRailItemIcon(projectButton)?.textContent).toContain('🧪');

    await clickAsync(allItemsButton);
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'All Items' })).toBeDefined();
      expect(getMainTitleIcon()).not.toBeNull();
    });

    await clickAsync(inboxButton);
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Untagged' })).toBeDefined();
      expect(getMainTitleIcon()).not.toBeNull();
    });

    await clickAsync(projectButton);
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Alpha' })).toBeDefined();
      expect(getMainTitleIcon()).not.toBeNull();
      expect(getMainTitleIcon()?.textContent).toContain('🧪');
    });
  });

  it('uses one contextual toolbar area for default controls and selection actions', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(screen.getByTestId('projects-toolbar-default')).toBeDefined();
    expect(screen.queryByTestId('projects-fab')).toBeNull();
    expect(screen.getByRole('textbox', { name: 'Search Untagged...' })).toBeDefined();
    expect(screen.queryByTestId('projects-results-count')).toBeNull();
    expect(screen.getByRole('button', { name: 'Filter' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open File Directory' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));

    expect(screen.getByTestId('projects-fab')).toBeDefined();
    expect(screen.getByText('0 selected')).toBeDefined();
    expect(screen.queryByTestId('projects-results-count')).toBeNull();
  });

  it('creates a project from the modal and makes it the active browse scope', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);

    fireEvent.click(screen.getByRole('button', { name: 'New Tag' }));
    fireEvent.change(screen.getByPlaceholderText('Tag name'), {
      target: { value: 'New Workspace' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create Tag' }));

    await waitFor(() => {
      expect(projectService.create).toHaveBeenCalled();
      expect(projectService.setActiveProjectId).toHaveBeenCalledWith('project-new');
    });
  });

  it('keeps project settings in a drawer and saves edits', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const updateProjectSpy = vi.spyOn(useProjectStore.getState(), 'updateProject');

    render(<ProjectsView />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    fireEvent.change(await screen.findByDisplayValue('Alpha'), {
      target: { value: 'Alpha Updated' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(updateProjectSpy).toHaveBeenCalledWith(
        'project-1',
        expect.objectContaining({ name: 'Alpha Updated', icon: '🧪' }),
      );
    });

    expect(screen.queryByText('Tag settings')).toBeNull();
  });

  it('keeps Tag settings metadata-only and does not write processing defaults', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const updateProjectSpy = vi.spyOn(useProjectStore.getState(), 'updateProject');

    render(<ProjectsView />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    expect(screen.queryByRole('checkbox', { name: 'Brand Terms' })).toBeNull();
    fireEvent.change(await screen.findByDisplayValue('Project alpha'), {
      target: { value: 'Metadata only' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(updateProjectSpy).toHaveBeenCalledWith(
        'project-1',
        expect.objectContaining({
          description: 'Metadata only',
        }),
      );
    });
    expect(updateProjectSpy.mock.calls[0]?.[1]).not.toHaveProperty('defaults');
  });

  it('guards switching to Inbox when project settings drafts are dirty', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const confirmSpy = vi.fn()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);
    useDialogStore.setState({
      ...useDialogStore.getState(),
      confirm: confirmSpy,
    });

    render(<ProjectsView />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    fireEvent.click(await screen.findByRole('button', { name: '🧪' }));
    fireEvent.click(await screen.findByRole('button', { name: '📄' }));

    fireEvent.click(getButtonByContent('Untagged'));
    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledTimes(1);
    });
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
    expect(screen.getByText('Tag settings')).toBeDefined();

    fireEvent.click(getButtonByContent('Untagged'));
    await waitFor(() => {
      expect(useProjectStore.getState().activeProjectId).toBeNull();
    });
    expect(confirmSpy).toHaveBeenCalledTimes(2);
    expect(screen.queryByText('Tag settings')).toBeNull();
  });

  it('guards closing project settings when icon-only edits are dirty', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const confirmSpy = vi.fn()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);
    useDialogStore.setState({
      ...useDialogStore.getState(),
      confirm: confirmSpy,
    });

    render(<ProjectsView />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    fireEvent.click(await screen.findByRole('button', { name: '🧪' }));
    fireEvent.click(await screen.findByRole('button', { name: '📄' }));

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByText('Tag settings')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => {
      expect(screen.queryByText('Tag settings')).toBeNull();
    });
    expect(confirmSpy).toHaveBeenCalledTimes(2);
  });

  it('rehydrates the active project icon after discarding and switching projects', async () => {
    useProjectStore.setState({
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: 'Project alpha',
          icon: '🧪',
          createdAt: 1,
          updatedAt: 1,
        },
        {
          id: 'project-2',
          name: 'Beta',
          description: 'Project beta',
          icon: '🎯',
          createdAt: 2,
          updatedAt: 2,
        },
      ],
      activeProjectId: 'project-1',
    });

    render(<ProjectsView />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    fireEvent.click(await screen.findByRole('button', { name: '🧪' }));
    fireEvent.click(await screen.findByRole('button', { name: '📄' }));

    await clickAsync(getButtonByContent('Beta'));
    await waitFor(() => {
      expect(useProjectStore.getState().activeProjectId).toBe('project-2');
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Tag Settings' }));
      await Promise.resolve();
    });
    expect(await screen.findByRole('button', { name: '🎯' })).toBeDefined();
    expect(screen.queryByRole('button', { name: '📄' })).toBeNull();
  });

  it('shows all history in All Items with global summary and project badges', async () => {
    const now = Date.now();
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-project',
          title: 'Project Item',
          timestamp: now - 1000,
          duration: 180,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-project.json',
          previewText: 'Project preview',
          type: 'recording',
          projectId: 'project-1',
        },
        {
          id: 'hist-inbox',
          title: 'Inbox Item',
          timestamp: now,
          duration: 240,
          audioPath: 'audio-2.wav',
          transcriptPath: 'hist-inbox.json',
          previewText: 'Inbox preview',
          type: 'batch',
          projectId: null,
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    await clickAsync(getButtonByContent('All Items'));
    await waitFor(() => {
      expect(screen.getByRole('textbox', { name: 'Search All Items...' })).toBeDefined();
    });

    expect(screen.getByTestId('projects-summary-chips')).toBeDefined();
    expect(screen.getByText('Project Item')).toBeDefined();
    expect(screen.getByText('Inbox Item')).toBeDefined();
    expect(screen.getByText('Project project-1')).toBeDefined();
    expect(screen.getByText('Project inbox')).toBeDefined();
    expect(screen.getByRole('textbox', { name: 'Search All Items...' })).toBeDefined();
    expect(screen.getByTestId('projects-summary-total-items').textContent).toBe('2');
    expect(screen.getByTestId('projects-summary-type-split').textContent).toBe('1 recordings / 1 imports');
    expect(screen.queryByTestId('projects-results-count')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Tag Settings' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Start Live Record' })).toBeNull();
  });

  it('opens an item from All Items, restores project context, and keeps All Items active', async () => {
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-project',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-project.json',
          previewText: 'Preview',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const allItemsButton = getButtonByContent('All Items');
    await clickAsync(allItemsButton);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Project Item' })).toBeDefined();
    });
    await clickAsync(screen.getByRole('button', { name: 'Project Item' }));

    await waitFor(() => {
      expect(getDetailPlaceholder()).not.toBeNull();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-project');
      expect(useProjectStore.getState().activeProjectId).toBe('project-1');
    });

    expect(allItemsButton.className).toContain('active');
    expect(screen.queryByRole('button', { name: 'Tag Settings' })).toBeNull();
  });

  it('opens a project item in the detail pane and closes it when switching out of scope', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Preview',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    await clickAsync(screen.getByRole('button', { name: 'Project Item' }));

    await waitFor(() => {
      expect(getDetailPlaceholder()).not.toBeNull();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-1');
    });

    await clickAsync(getButtonByContent('Untagged'));

    await waitFor(() => {
      expect(getDetailPlaceholder()).toBeNull();
      expect(useTranscriptStore.getState().sourceHistoryId).toBeNull();
    });
  });

  it('closes the workspace detail pane when the active transcript is cleared externally', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Preview',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    await act(async () => {
      useTranscriptStore.getState().loadTranscript(
        [{ id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true }],
        'hist-1',
        'Project Item',
      );
      useTranscriptStore.getState().setAudioUrl('asset:///audio.wav');
    });

    await waitFor(() => {
      expect(getDetailPlaceholder()).not.toBeNull();
    });

    await act(async () => {
      useTranscriptStore.getState().clearSegments();
    });

    await waitFor(() => {
      expect(getDetailPlaceholder()).toBeNull();
      expect(screen.getByRole('textbox', { name: 'Search in Alpha...' })).toBeDefined();
    });
  });

  it('removes a tag from selected items without replacing other assignments', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Preview',
          projectId: 'project-1',
        },
      ],
    } as any);

    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);

    await screen.findByText('Project Item');
    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-1' }));
    fireEvent.click(screen.getByRole('button', { name: 'Edit Tags' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Alpha' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(historyService.updateTagAssignments).toHaveBeenCalledWith(
        ['hist-1'],
        [],
        ['project-1'],
      );
    });
  });

  it('keeps Trash items read-only and supports restore and permanent deletion', async () => {
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-trash',
          title: 'Discarded Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-trash.json',
          previewText: 'Preview',
          tagIds: ['project-1'],
          deletedAt: Date.now(),
        },
      ],
    } as any);
    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();
    await clickAsync(getButtonByContent('Trash'));

    const historyItem = await screen.findByTestId('history-item-hist-trash');
    expect(historyItem.dataset.loadDisabled).toBe('true');
    expect(historyItem.dataset.renameDisabled).toBe('true');

    fireEvent.contextMenu(historyItem, { clientX: 160, clientY: 220 });
    expect(screen.getByRole('menuitem', { name: 'Restore' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Delete Permanently' })).toBeDefined();
    expect(screen.queryByRole('menuitem', { name: 'Open' })).toBeNull();

    fireEvent.click(screen.getByRole('menuitem', { name: 'Restore' }));
    await waitFor(() => {
      expect(historyService.restoreRecordings).toHaveBeenCalledWith(['hist-trash']);
    });
    await waitFor(() => {
      expect(screen.queryByRole('menu')).toBeNull();
    });

    fireEvent.contextMenu(screen.getByTestId('history-item-hist-trash'), { clientX: 160, clientY: 220 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete Permanently' }));
    await waitFor(() => {
      expect(historyService.purgeRecordings).toHaveBeenCalledWith(['hist-trash']);
    });
  });

  it('empties every item currently in Trash', async () => {
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-trash-a',
          title: 'Discarded A',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio-a.wav',
          transcriptPath: 'hist-trash-a.json',
          previewText: 'Preview',
          deletedAt: Date.now(),
        },
        {
          id: 'hist-trash-b',
          title: 'Discarded B',
          timestamp: Date.now() - 1,
          duration: 12,
          audioPath: 'audio-b.wav',
          transcriptPath: 'hist-trash-b.json',
          previewText: 'Preview',
          deletedAt: Date.now(),
        },
      ],
    } as any);
    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();
    await clickAsync(getButtonByContent('Trash'));

    await screen.findByTestId('history-item-hist-trash-a');
    fireEvent.click(screen.getByRole('button', { name: 'Empty Trash' }));

    await waitFor(() => {
      expect(historyService.purgeRecordings).toHaveBeenCalledWith(['hist-trash-a', 'hist-trash-b']);
    });
  });

  it('supports bulk delete and opening the file directory from the workspace toolbar', async () => {
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-project',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-project.json',
          previewText: 'Preview',
          projectId: 'project-1',
        },
      ],
    } as any);

    const { historyService } = await import('../../services/historyService');

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    await clickAsync(getButtonByContent('All Items'));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Project Item' })).toBeDefined();
    });
    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-project' }));
    await clickAsync(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(historyService.deleteRecordings).toHaveBeenCalledWith(['hist-project']);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Open File Directory' }));
    expect(historyService.openHistoryFolder).toHaveBeenCalled();
  });

  it('jumps into live mode while keeping the active project context', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'Start Live Record' }));

    expect(useTranscriptStore.getState().mode).toBe('live');
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
  });

  it('searches, filters, and sorts only within the current browse scope', async () => {
    const now = Date.now();
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const items = [
      {
        id: 'hist-recording',
        title: 'Client Call',
        timestamp: now - (2 * 24 * 60 * 60 * 1000),
        duration: 180,
        audioPath: 'audio-1.wav',
        transcriptPath: 'hist-recording.json',
        previewText: 'Quarterly planning',
        searchContent: 'Quarterly roadmap follow up',
        type: 'recording',
        projectId: 'project-1',
      },
      {
        id: 'hist-batch-old',
        title: 'Imported Deck',
        timestamp: now - (10 * 24 * 60 * 60 * 1000),
        duration: 320,
        audioPath: 'audio-2.wav',
        transcriptPath: 'hist-batch-old.json',
        previewText: 'Slides transcript',
        searchContent: 'Deck transcript import',
        type: 'batch',
        projectId: 'project-1',
      },
      {
        id: 'hist-batch-recent',
        title: 'Workshop Import',
        timestamp: now - (24 * 60 * 60 * 1000),
        duration: 240,
        audioPath: 'audio-3.wav',
        transcriptPath: 'hist-batch-recent.json',
        previewText: 'Workshop notes',
        searchContent: 'Training import summary',
        type: 'batch',
        projectId: 'project-1',
      },
      {
        id: 'hist-inbox',
        title: 'Inbox Item',
        timestamp: now,
        duration: 60,
        audioPath: 'audio-4.wav',
        transcriptPath: 'hist-inbox.json',
        previewText: 'Inbox only',
        searchContent: 'Should never appear',
        type: 'recording',
        projectId: null,
      },
    ];
    useHistoryStore.setState({ items } as any);
    const projectItems = items.filter((item) => item.projectId === 'project-1');
    workspaceQueryBackendMock.impl = ({ query, filterType, dateFilter }) => {
      if (query === 'roadmap') {
        return buildWorkspaceQueryResult({
          filteredItems: [items[0]],
          scopedItems: projectItems,
          allItems: items,
          searchMatchByItemId: {
            'hist-recording': makeSearchMatch('Quarterly roadmap follow up'),
          },
        });
      }

      if (filterType === 'batch' && dateFilter === 'week') {
        return buildWorkspaceQueryResult({
          filteredItems: [items[2]],
          scopedItems: projectItems,
          allItems: items,
        });
      }

      if (filterType === 'batch') {
        return buildWorkspaceQueryResult({
          filteredItems: [items[1], items[2]],
          scopedItems: projectItems,
          allItems: items,
        });
      }

      return buildWorkspaceQueryResult({
        filteredItems: projectItems,
        scopedItems: projectItems,
        allItems: items,
      });
    };

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.change(screen.getByRole('textbox', { name: 'Search in Alpha...' }), {
      target: { value: 'roadmap' },
    });

    await waitFor(() => {
      expect(screen.getByText('Client Call')).toBeDefined();
      expect(screen.queryByText('Imported Deck')).toBeNull();
      expect(screen.getByText('Query roadmap')).toBeDefined();
      expect(screen.getByText(/Snippet Quarterly roadmap follow up/i)).toBeDefined();
      expect(screen.queryByText('Inbox Item')).toBeNull();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));

    openFilterMenu();
    selectDropdownOption('Filter by type', 'Batch imports');
    await waitFor(() => {
      expect(screen.getAllByText(/Batch imports/i).length).toBeGreaterThan(0);
      expect(screen.queryByText('Client Call')).toBeNull();
      expect(screen.getByText('Imported Deck')).toBeDefined();
      expect(screen.getByText('Workshop Import')).toBeDefined();
    });

    openFilterMenu();
    selectDropdownOption('Filter by date', 'Last 7 days');
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Filter' }).textContent).toContain('2');
      expect(screen.queryByText('Imported Deck')).toBeNull();
      expect(screen.getByText('Workshop Import')).toBeDefined();
    });

    selectDropdownOption('Sort items', 'Title A-Z');
    await waitFor(() => {
      const orderedItems = screen.getAllByTestId(/history-item-/).map((item) => item.textContent || '');
      expect(orderedItems[0]).toContain('Workshop Import');
    });
  });

  it('opens the filter popover and clears active filters without affecting sort controls', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const items = [
      {
        id: 'hist-recording',
        title: 'Client Call',
        timestamp: Date.now() - (2 * 24 * 60 * 60 * 1000),
        duration: 180,
        audioPath: 'audio-1.wav',
        transcriptPath: 'hist-recording.json',
        previewText: 'Quarterly planning',
        searchContent: 'Quarterly roadmap follow up',
        type: 'recording',
        projectId: 'project-1',
      },
      {
        id: 'hist-batch',
        title: 'Workshop Import',
        timestamp: Date.now() - (24 * 60 * 60 * 1000),
        duration: 240,
        audioPath: 'audio-2.wav',
        transcriptPath: 'hist-batch.json',
        previewText: 'Workshop notes',
        searchContent: 'Training import summary',
        type: 'batch',
        projectId: 'project-1',
      },
    ];
    useHistoryStore.setState({ items } as any);
    workspaceQueryBackendMock.impl = ({ filterType }) => buildWorkspaceQueryResult({
      filteredItems: filterType === 'batch' ? [items[1]] : items,
      scopedItems: items,
      allItems: items,
    });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    openFilterMenu();
    expect(screen.getByText('Refine the current workspace view by type or time.')).toBeDefined();

    selectDropdownOption('Filter by type', 'Batch imports');
    await waitFor(() => {
      expect(screen.queryByText('Client Call')).toBeNull();
      expect(screen.getAllByText(/Batch imports/i).length).toBeGreaterThan(0);
    });

    openFilterMenu();
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));

    expect(screen.getByText(/All items/i)).toBeDefined();
    await waitFor(() => {
      expect(screen.getByText('Client Call')).toBeDefined();
    });
    expect(screen.getByRole('button', { name: 'Sort items' })).toBeDefined();
  });

  it('focuses the workspace search with Ctrl+F when the detail pane is not focused', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const input = screen.getByRole('textbox', { name: 'Search Untagged...' });
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });

    expect(document.activeElement).toBe(input);
  });

  it('does not steal Ctrl+F when focus is already inside the detail pane', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-project',
          title: 'Project Item',
          timestamp: Date.now(),
          duration: 120,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-project.json',
          previewText: 'Roadmap preview',
          searchContent: 'Roadmap preview',
          type: 'recording',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'Project Item' }));
    await waitFor(() => {
      expect(getDetailPlaceholder()).not.toBeNull();
    });

    const detailHost = document.createElement('div');
    detailHost.className = 'projects-detail-pane';
    const detailButton = document.createElement('button');
    detailButton.textContent = 'Detail Focus';
    detailHost.appendChild(detailButton);
    document.body.appendChild(detailHost);
    detailButton.focus();
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });

    expect(document.activeElement).toBe(detailButton);
    detailHost.remove();
  });

  it('navigates workspace search results with arrow keys and opens the active result on Enter', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Alpha Plan',
          timestamp: Date.now(),
          duration: 120,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Roadmap preview',
          searchContent: 'Roadmap preview',
          type: 'recording',
          projectId: 'project-1',
        },
        {
          id: 'hist-2',
          title: 'Beta Plan',
          timestamp: Date.now() - 1000,
          duration: 90,
          audioPath: 'audio-2.wav',
          transcriptPath: 'hist-2.json',
          previewText: 'Roadmap notes',
          searchContent: 'Roadmap notes',
          type: 'recording',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const input = screen.getByRole('textbox', { name: 'Search in Alpha...' });
    fireEvent.change(input, { target: { value: 'roadmap' } });
    await waitFor(() => {
      expect(screen.getByTestId('history-item-hist-1')).toBeDefined();
    });
    fireEvent.keyDown(input, { key: 'ArrowDown' });

    expect(screen.getByTestId('history-item-hist-1').getAttribute('data-keyboard-active')).toBe('true');

    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getByTestId('history-item-hist-2').getAttribute('data-keyboard-active')).toBe('true');

    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(getDetailPlaceholder()).not.toBeNull();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-2');
    });
  });

  it('refreshes workspace search results and snippets immediately after a transcript metadata sync', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const item = {
      id: 'hist-1',
      title: 'Alpha Plan',
      timestamp: Date.now(),
      duration: 120,
      audioPath: 'audio-1.wav',
      transcriptPath: 'hist-1.json',
      previewText: 'Meeting notes',
      searchContent: 'Meeting notes',
      type: 'recording',
      projectId: 'project-1',
    };
    useHistoryStore.setState({ items: [item] } as any);
    let roadmapQueryCount = 0;
    workspaceQueryBackendMock.impl = ({ query }, currentItems) => {
      if (query === 'roadmap') {
        roadmapQueryCount += 1;
        if (roadmapQueryCount === 1) {
          return buildWorkspaceQueryResult({
            filteredItems: [],
            scopedItems: currentItems,
            allItems: currentItems,
          });
        }

        return buildWorkspaceQueryResult({
          filteredItems: currentItems,
          scopedItems: currentItems,
          allItems: currentItems,
          searchMatchByItemId: {
            'hist-1': makeSearchMatch('Fresh roadmap notes...'),
          },
        });
      }

      return buildWorkspaceQueryResult({
        filteredItems: currentItems,
        scopedItems: currentItems,
        allItems: currentItems,
      });
    };

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const input = screen.getByRole('textbox', { name: 'Search in Alpha...' });
    fireEvent.change(input, { target: { value: 'roadmap' } });

    await waitFor(() => {
      expect(screen.getByText('No matching items')).toBeDefined();
    });

    await act(async () => {
      await useHistoryStore.getState().updateTranscript('hist-1', [
        { id: 'seg-1', start: 0, end: 1, text: 'Fresh roadmap notes', isFinal: true },
      ]);
    });

    await waitFor(() => {
      expect(screen.queryByText('No matching items')).toBeNull();
      expect(screen.getByRole('button', { name: 'Alpha Plan' })).toBeDefined();
      expect(screen.getByText('Snippet Fresh roadmap notes...')).toBeDefined();
    });
  });

  it('disables active-result keyboard navigation while selection mode is enabled', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Alpha Plan',
          timestamp: Date.now(),
          duration: 120,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Roadmap preview',
          searchContent: 'Roadmap preview',
          type: 'recording',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    const input = screen.getByRole('textbox', { name: 'Search in Alpha...' });
    fireEvent.change(input, { target: { value: 'roadmap' } });
    await waitFor(() => {
      expect(screen.getByTestId('history-item-hist-1')).toBeDefined();
    });
    fireEvent.keyDown(input, { key: 'ArrowDown' });

    expect(screen.getByTestId('history-item-hist-1').getAttribute('data-keyboard-active')).toBe('false');
  });

  it('clears the query on first Escape and blurs the search box on second Escape', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
        {
          id: 'hist-1',
          title: 'Alpha Plan',
          timestamp: Date.now(),
          duration: 120,
          audioPath: 'audio-1.wav',
          transcriptPath: 'hist-1.json',
          previewText: 'Roadmap preview',
          searchContent: 'Roadmap preview',
          type: 'recording',
          projectId: 'project-1',
        },
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    const input = screen.getByRole('textbox', { name: 'Search in Alpha...' }) as HTMLInputElement;
    input.focus();
    fireEvent.change(input, { target: { value: 'roadmap' } });
    fireEvent.keyDown(input, { key: 'Escape' });

    expect(input.value).toBe('');
    expect(document.activeElement).toBe(input);

    fireEvent.keyDown(input, { key: 'Escape' });
    expect(document.activeElement).not.toBe(input);
  });

  it('shows a no-results state and trims hidden selections without dropping the open detail pane', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const items = [
      {
        id: 'hist-1',
        title: 'Project Item',
        timestamp: Date.now() - 500,
        duration: 120,
        audioPath: 'audio-1.wav',
        transcriptPath: 'hist-1.json',
        previewText: 'Preview',
        searchContent: 'Preview',
        type: 'recording',
        projectId: 'project-1',
      },
      {
        id: 'hist-2',
        title: 'Batch Item',
        timestamp: Date.now() - 1000,
        duration: 360,
        audioPath: 'audio-2.wav',
        transcriptPath: 'hist-2.json',
        previewText: 'Batch preview',
        searchContent: 'Batch preview',
        type: 'batch',
        projectId: 'project-1',
      },
    ];
    useHistoryStore.setState({ items } as any);
    workspaceQueryBackendMock.impl = ({ query }) => buildWorkspaceQueryResult({
      filteredItems: query === 'missing item' ? [] : items,
      scopedItems: items,
      allItems: items,
    });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-1' }));

    fireEvent.change(screen.getByRole('textbox', { name: 'Search in Alpha...' }), {
      target: { value: 'missing item' },
    });

    await waitFor(() => {
      expect(screen.getByText('No matching items')).toBeDefined();
      expect(screen.queryByText('No items in this workspace yet.')).toBeNull();
      expect(screen.getByRole('button', { name: 'Edit Tags' }).hasAttribute('disabled')).toBe(true);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Project Item' })).toBeDefined();
    });
  });
});
