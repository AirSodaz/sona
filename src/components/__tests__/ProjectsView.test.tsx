import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ProjectsView } from '../ProjectsView';
import { useProjectStore } from '../../stores/projectStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';

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

vi.mock('../../services/historyService', () => ({
  historyService: {
    getAll: vi.fn().mockResolvedValue([]),
    loadTranscript: vi.fn().mockResolvedValue([]),
    getAudioUrl: vi.fn().mockResolvedValue('asset:///audio.wav'),
    updateProjectAssignments: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../history/HistoryItem', () => ({
  HistoryItem: ({ item, onLoad, isSelectionMode, isSelected, onToggleSelection }: any) => (
    <div>
      <button onClick={() => onLoad(item)}>{item.title}</button>
      {isSelectionMode && (
        <button onClick={() => onToggleSelection?.(item.id)}>
          {isSelected ? `Selected ${item.id}` : `Select ${item.id}`}
        </button>
      )}
    </div>
  ),
}));

describe('ProjectsView', () => {
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
          id: 'hist-1',
          title: 'Inbox Item',
          timestamp: Date.now(),
          duration: 12,
          audioPath: 'audio.wav',
          transcriptPath: 'hist-1.json',
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

  it('creates a project and makes it active', async () => {
    const { projectService } = await import('../../services/projectService');

    render(<ProjectsView />);

    fireEvent.change(screen.getByPlaceholderText('projects.new_project_name'), {
      target: { value: 'New Workspace' },
    });
    fireEvent.click(screen.getAllByRole('button', { name: 'projects.create_action' })[0]);

    await waitFor(() => {
      expect(projectService.create).toHaveBeenCalled();
      expect(projectService.setActiveProjectId).toHaveBeenCalledWith('project-new');
    });
  });

  it('saves project edits and jumps into live mode from the workspace header', async () => {
    useProjectStore.setState({ activeProjectId: 'project-1' });
    const updateProjectSpy = vi.spyOn(useProjectStore.getState(), 'updateProject');

    render(<ProjectsView />);

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

    fireEvent.click(screen.getByRole('button', { name: 'projects.start_live_record' }));
    expect(useTranscriptStore.getState().mode).toBe('live');
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
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Select hist-1' })).toBeDefined();
    });
    fireEvent.click(screen.getByRole('button', { name: 'Select hist-1' }));
    fireEvent.click(screen.getByRole('button', { name: 'projects.move_selected' }));

    await waitFor(() => {
      expect(historyService.updateProjectAssignments).toHaveBeenCalledWith(['hist-1'], null);
    });
  });
});
