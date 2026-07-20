import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../configStore';
import { useProjectStore } from '../projectStore';

vi.mock('../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn(),
    getActiveProjectId: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    setActiveProjectId: vi.fn(),
  },
}));

vi.mock('../../services/historyService', () => ({
  historyService: {
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

describe('projectStore', () => {
  beforeEach(() => {
    useProjectStore.setState({
      projects: [],
      activeProjectId: null,
      isLoading: false,
      error: null,
    });
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand', enabled: true, keywords: 'Sona' },
          { id: 'kw-2', name: 'Style', enabled: false, keywords: 'Sentence case' },
        ],
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
        ],
      },
    });
    vi.clearAllMocks();
  });

  it('loads persisted projects and active project id', async () => {
    const { projectService } = await import('../../services/projectService');
    (projectService.getAll as any).mockResolvedValue([
      {
        id: 'project-1',
        name: 'Alpha',
        description: '',
        createdAt: 1,
        updatedAt: 1,
      },
    ]);
    (projectService.getActiveProjectId as any).mockResolvedValue('project-1');

    await useProjectStore.getState().loadProjects();

    expect(projectService.getAll).toHaveBeenCalledWith();
    expect(useProjectStore.getState().projects).toHaveLength(1);
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
  });

  it('creates a metadata-only Tag without copying global config', async () => {
    const { projectService } = await import('../../services/projectService');
    (projectService.create as any).mockImplementation(async ({ name, description }: any) => ({
      id: 'project-1',
      name,
      description,
      createdAt: 1,
      updatedAt: 1,
    }));

    const project = await useProjectStore.getState().createProject({ name: 'Alpha' });

    expect(project?.name).toBe('Alpha');
    expect(useProjectStore.getState().projects).toHaveLength(1);
  });

  it('deletes the active project and moves its history back to Inbox', async () => {
    const { projectService } = await import('../../services/projectService');
    const { historyService } = await import('../../services/historyService');

    useProjectStore.setState({
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: '',
          icon: '',
          createdAt: 1,
          updatedAt: 1,
        },
      ],
      activeProjectId: 'project-1',
    });

    await useProjectStore.getState().deleteProject('project-1');

    expect(historyService.updateProjectAssignmentsByCurrentProject).not.toHaveBeenCalled();
    expect(projectService.delete).toHaveBeenCalledWith('project-1');
    expect(projectService.setActiveProjectId).toHaveBeenCalledWith(null);
    expect(useProjectStore.getState().activeProjectId).toBeNull();
  });

  it('assigns selected history items to a project', async () => {
    const { historyService } = await import('../../services/historyService');

    await useProjectStore.getState().assignHistoryItems(['hist-1', 'hist-2'], 'project-2');

    expect(historyService.updateProjectAssignments).toHaveBeenCalledWith(['hist-1', 'hist-2'], 'project-2');
  });
});
