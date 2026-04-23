import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  },
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
  HistoryItem: ({ item, onLoad, isSelectionMode, isSelected, onToggleSelection }: any) => (
    <div data-testid={`history-item-${item.id}`}>
      <button onClick={() => onLoad(item)}>{item.title}</button>
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
      if (key === 'projects.items_title' && options?.count !== undefined) {
        return `${options.count} items`;
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
  const waitForInitialHistoryLoad = async () => {
    const { historyService } = await import('../../services/historyService');

    await waitFor(() => {
      expect(historyService.getAll).toHaveBeenCalled();
      expect(useHistoryStore.getState().isLoading).toBe(false);
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

  it('creates a project from the create modal and makes it active', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);

    fireEvent.click(screen.getAllByRole('button', { name: 'projects.new_project_button' })[0]);
    fireEvent.change(screen.getByPlaceholderText('projects.new_project_name'), {
      target: { value: 'New Workspace' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'projects.create_action' }));

    await waitFor(() => {
      expect(projectService.create).toHaveBeenCalled();
      expect(projectService.setActiveProjectId).toHaveBeenCalledWith('project-new');
    });
  });

  it('keeps project settings in a drawer and saves edits', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const updateProjectSpy = vi.spyOn(useProjectStore.getState(), 'updateProject');

    render(<ProjectsView />);

    expect(screen.queryByText('projects.project_settings_title')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'projects.project_settings' }));
    expect(screen.getByText('projects.project_settings_title')).toBeDefined();

    fireEvent.change(screen.getByDisplayValue('Alpha'), {
      target: { value: 'Alpha Updated' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

    await waitFor(() => {
      expect(updateProjectSpy).toHaveBeenCalledWith(
        'project-1',
        expect.objectContaining({ name: 'Alpha Updated' }),
      );
    });
  });

  it('opens a project item in the built-in detail pane and closes it when switching scope', async () => {
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

    const projectItemButton = await screen.findByRole('button', { name: 'Project Item' });
    fireEvent.click(projectItemButton);

    await waitFor(() => {
      expect(screen.getByText('TranscriptEditor')).toBeDefined();
      expect(useTranscriptStore.getState().sourceHistoryId).toBe('hist-1');
    });

    const inboxButton = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('projects.inbox_description'));
    expect(inboxButton).not.toBeNull();
    fireEvent.click(inboxButton!);

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
    fireEvent.click(screen.getByRole('button', { name: 'common.select' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-1' }));
    fireEvent.click(screen.getByRole('button', { name: 'projects.move_selected' }));

    await waitFor(() => {
      expect(historyService.updateProjectAssignments).toHaveBeenCalledWith(['hist-1'], null);
    });
  });

  it('jumps into live mode while keeping the active project context', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });

    render(<ProjectsView />);
    await waitForInitialHistoryLoad();

    fireEvent.click(screen.getByRole('button', { name: 'projects.start_live_record' }));

    expect(useTranscriptStore.getState().mode).toBe('live');
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
  });
});
