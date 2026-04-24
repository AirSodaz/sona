import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ProjectsView } from '../ProjectsView';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptStore } from '../../stores/transcriptStore';

vi.mock('../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn().mockResolvedValue([]),
    getActiveProjectId: vi.fn().mockResolvedValue(null),
    create: vi.fn().mockImplementation(async ({ name, description, defaults }: any) => ({
      id: 'project-new',
      name,
      description,
      createdAt: 1,
      updatedAt: 1,
      defaults,
    })),
    update: vi.fn().mockImplementation(async (id: string, updates: any) => ({
      id,
      name: updates.name || 'Alpha',
      description: updates.description || '',
      createdAt: 1,
      updatedAt: 2,
      defaults: updates.defaults,
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
    updateProjectAssignments: vi.fn().mockResolvedValue(undefined),
    deleteRecording: vi.fn().mockResolvedValue(undefined),
    deleteRecordings: vi.fn().mockResolvedValue(undefined),
    openHistoryFolder: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../TranscriptWorkbench', () => ({
  TranscriptWorkbench: ({ title, onClose }: any) => (
    <div>
      <div>TranscriptEditor</div>
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
  HistoryItem: ({ item, onLoad, isSelectionMode, isSelected, onToggleSelection, searchQuery }: any) => (
    <div data-testid={`history-item-${item.id}`} data-search-query={searchQuery || ''}>
      <button onClick={() => onLoad(item)}>{item.title}</button>
      <span>{`Project ${item.projectId ?? 'inbox'}`}</span>
      {searchQuery && <span>{`Query ${searchQuery}`}</span>}
      {isSelected && <span>{`Active ${item.id}`}</span>}
      {isSelectionMode && (
        <button onClick={() => onToggleSelection?.(item.id)}>
          {isSelected ? `Selected ${item.id}` : `Select ${item.id}`}
        </button>
      )}
    </div>
  ),
}));

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
    const button = screen.getAllByRole('button').find((candidate) => candidate.textContent?.includes(label));
    expect(button).not.toBeNull();
    return button as HTMLButtonElement;
  };

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

  beforeEach(async () => {
    vi.clearAllMocks();

    useProjectStore.setState({
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: 'Project alpha',
          createdAt: 1,
          updatedAt: 1,
          defaults: {
            summaryTemplate: 'general',
            translationLanguage: 'zh',
            polishScenario: 'custom',
            polishContext: '',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
          },
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
        polishScenario: 'lecture',
        polishContext: 'Context',
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
    expect(getButtonByContent('Inbox')).toBeDefined();
    expect(screen.getAllByRole('button', { name: 'New Project' })).toHaveLength(1);
  });

  it('uses one contextual toolbar area for default controls and selection actions', async () => {
    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    expect(screen.getByTestId('projects-toolbar-default')).toBeDefined();
    expect(screen.queryByTestId('projects-fab')).toBeNull();
    expect(screen.getByTestId('projects-results-count').textContent).toBe('Showing 1 of 1');
    expect(screen.getByRole('button', { name: 'Filter' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open File Directory' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));

    expect(screen.getByTestId('projects-fab')).toBeDefined();
    expect(screen.getByText('0 selected')).toBeDefined();
    expect(screen.getByTestId('projects-results-count').textContent).toBe('Showing 1 of 1');
  });

  it('creates a project from the modal and makes it the active browse scope', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);

    fireEvent.click(screen.getByRole('button', { name: 'New Project' }));
    fireEvent.change(screen.getByPlaceholderText('Project name'), {
      target: { value: 'New Workspace' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create Project' }));

    await waitFor(() => {
      expect(projectService.create).toHaveBeenCalled();
      expect(projectService.setActiveProjectId).toHaveBeenCalledWith('project-new');
    });
  });

  it('keeps project settings in a drawer and saves edits', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const updateProjectSpy = vi.spyOn(useProjectStore.getState(), 'updateProject');

    render(<ProjectsView />);

    fireEvent.click(screen.getByRole('button', { name: 'Project Settings' }));
    fireEvent.change(screen.getByDisplayValue('Alpha'), {
      target: { value: 'Alpha Updated' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(updateProjectSpy).toHaveBeenCalledWith(
        'project-1',
        expect.objectContaining({ name: 'Alpha Updated' }),
      );
    });

    expect(screen.queryByText('Edit Project Defaults')).toBeNull();
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

    fireEvent.click(screen.getByRole('button', { name: 'Project Settings' }));
    fireEvent.change(screen.getByDisplayValue('Alpha'), {
      target: { value: 'Alpha Updated' },
    });

    fireEvent.click(getButtonByContent('Inbox'));
    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledTimes(1);
    });
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
    expect(screen.getByText('Edit Project Defaults')).toBeDefined();

    fireEvent.click(getButtonByContent('Inbox'));
    await waitFor(() => {
      expect(useProjectStore.getState().activeProjectId).toBeNull();
    });
    expect(confirmSpy).toHaveBeenCalledTimes(2);
    expect(screen.queryByText('Edit Project Defaults')).toBeNull();
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
      expect(screen.getByTestId('projects-results-count').textContent).toBe('Showing 2 of 2');
    });

    expect(screen.getByTestId('projects-summary-chips')).toBeDefined();
    expect(screen.getByText('Project Item')).toBeDefined();
    expect(screen.getByText('Inbox Item')).toBeDefined();
    expect(screen.getByText('Project project-1')).toBeDefined();
    expect(screen.getByText('Project inbox')).toBeDefined();
    expect(screen.getByTestId('projects-summary-total-items').textContent).toBe('2');
    expect(screen.getByTestId('projects-summary-type-split').textContent).toBe('1 recordings / 1 imports');
    expect(screen.getByTestId('projects-results-count').textContent).toBe('Showing 2 of 2');
    expect(screen.queryByRole('button', { name: 'Project Settings' })).toBeNull();
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
      expect(screen.getByText('TranscriptEditor')).toBeDefined();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-project');
      expect(useProjectStore.getState().activeProjectId).toBe('project-1');
    });

    expect(allItemsButton.className).toContain('active');
    expect(screen.queryByRole('button', { name: 'Project Settings' })).toBeNull();
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
      expect(screen.getByText('TranscriptEditor')).toBeDefined();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-1');
    });

    await clickAsync(getButtonByContent('Inbox'));

    await waitFor(() => {
      expect(screen.queryByText('TranscriptEditor')).toBeNull();
      expect(useTranscriptStore.getState().sourceHistoryId).toBeNull();
    });
  });

  it('moves selected items from the active project back to Inbox', async () => {
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
    fireEvent.click(screen.getByRole('button', { name: 'Move Selected' }));

    await waitFor(() => {
      expect(historyService.updateProjectAssignments).toHaveBeenCalledWith(['hist-1'], null);
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
    useHistoryStore.setState({
      items: [
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
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.change(screen.getByRole('textbox', { name: 'Search this workspace...' }), {
      target: { value: 'roadmap' },
    });

    expect(screen.getByText('Client Call')).toBeDefined();
    expect(screen.queryByText('Imported Deck')).toBeNull();
    expect(screen.getByText('Query roadmap')).toBeDefined();
    expect(screen.queryByText('Inbox Item')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));

    openFilterMenu();
    selectDropdownOption('Filter by type', 'Batch imports');
    expect(screen.getAllByText(/Batch imports/i).length).toBeGreaterThan(0);
    expect(screen.queryByText('Client Call')).toBeNull();
    expect(screen.getByText('Imported Deck')).toBeDefined();
    expect(screen.getByText('Workshop Import')).toBeDefined();

    openFilterMenu();
    selectDropdownOption('Filter by date', 'Last 7 days');
    expect(screen.getByRole('button', { name: 'Filter' }).textContent).toContain('2');
    expect(screen.queryByText('Imported Deck')).toBeNull();
    expect(screen.getByText('Workshop Import')).toBeDefined();

    selectDropdownOption('Sort items', 'Title A-Z');
    const orderedItems = screen.getAllByTestId(/history-item-/).map((item) => item.textContent || '');
    expect(orderedItems[0]).toContain('Workshop Import');
  });

  it('opens the filter popover and clears active filters without affecting sort controls', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
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
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    openFilterMenu();
    expect(screen.getByText('Refine the current workspace view by type or time.')).toBeDefined();

    selectDropdownOption('Filter by type', 'Batch imports');
    expect(screen.queryByText('Client Call')).toBeNull();
    expect(screen.getAllByText(/Batch imports/i).length).toBeGreaterThan(0);

    openFilterMenu();
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));

    expect(screen.getByText(/All items/i)).toBeDefined();
    expect(screen.getByText('Client Call')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Sort items' })).toBeDefined();
  });

  it('shows a no-results state and trims hidden selections without dropping the open detail pane', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    useHistoryStore.setState({
      items: [
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
      ],
    } as any);

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-1' }));

    fireEvent.change(screen.getByRole('textbox', { name: 'Search this workspace...' }), {
      target: { value: 'missing item' },
    });

    expect(screen.getByText('No matching items')).toBeDefined();
    expect(screen.queryByText('No items in this workspace yet.')).toBeNull();
    expect(screen.getByRole('button', { name: 'Move Selected' }).hasAttribute('disabled')).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(screen.getByRole('button', { name: 'Project Item' })).toBeDefined();
  });
});
