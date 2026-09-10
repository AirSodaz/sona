import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProjectRecord } from '../../types/project';
import { projectService } from '../projectService';

const tauriProjectMocks = vi.hoisted(() => ({
  projectCreate: vi.fn(),
  projectDelete: vi.fn(),
  projectGetActiveId: vi.fn(),
  projectList: vi.fn(),
  projectReorder: vi.fn(),
  projectSetActiveId: vi.fn(),
  projectUpdate: vi.fn(),
}));

vi.mock('../tauri/project', () => ({
  projectCreate: tauriProjectMocks.projectCreate,
  projectDelete: tauriProjectMocks.projectDelete,
  projectGetActiveId: tauriProjectMocks.projectGetActiveId,
  projectList: tauriProjectMocks.projectList,
  projectReorder: tauriProjectMocks.projectReorder,
  projectSetActiveId: tauriProjectMocks.projectSetActiveId,
  projectUpdate: tauriProjectMocks.projectUpdate,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  BaseDirectory: { AppLocalData: 3 },
  exists: vi.fn().mockResolvedValue(true),
  mkdir: vi.fn().mockResolvedValue(undefined),
  readTextFile: vi.fn().mockResolvedValue('[]'),
  writeTextFile: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../storageService', () => ({
  STORE_KEY_ACTIVE_PROJECT: 'sona-active-project-id',
  settingsStore: {
    get: vi.fn().mockResolvedValue(null),
    save: vi.fn().mockResolvedValue(undefined),
    set: vi.fn().mockResolvedValue(undefined),
  },
}));

describe('projectService', () => {
  const project: ProjectRecord = {
    id: 'project-1',
    name: 'Research',
    description: 'Notes',
    icon: 'folder',
    createdAt: 1,
    updatedAt: 1,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    tauriProjectMocks.projectList.mockResolvedValue([]);
    tauriProjectMocks.projectCreate.mockResolvedValue(project);
    tauriProjectMocks.projectUpdate.mockResolvedValue(project);
    tauriProjectMocks.projectDelete.mockResolvedValue(undefined);
    tauriProjectMocks.projectReorder.mockResolvedValue([project]);
    tauriProjectMocks.projectGetActiveId.mockResolvedValue(null);
    tauriProjectMocks.projectSetActiveId.mockResolvedValue(undefined);
  });

  it('delegates the public project API to the Rust repository wrapper', async () => {
    tauriProjectMocks.projectList
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([project]);
    tauriProjectMocks.projectGetActiveId.mockResolvedValueOnce('project-1');

    await projectService.init();
    const projects = await projectService.getAll();
    await projectService.saveAll([project]);
    const created = await projectService.create({
      name: 'Research',
      description: 'Notes',
      icon: 'folder',
    });
    const updated = await projectService.update('project-1', { name: 'Updated' });
    await projectService.delete('project-1');
    await projectService.reorder(['project-1']);
    const activeProjectId = await projectService.getActiveProjectId();
    await projectService.setActiveProjectId(null);

    expect(projects).toEqual([project]);
    expect(created).toEqual(project);
    expect(updated).toEqual(project);
    expect(activeProjectId).toBe('project-1');
    expect(tauriProjectMocks.projectList.mock.calls[0]).toEqual([]);
    expect(tauriProjectMocks.projectList).toHaveBeenNthCalledWith(2);
    expect(tauriProjectMocks.projectCreate).toHaveBeenCalledWith({
      name: 'Research',
      description: 'Notes',
      icon: 'folder',
    });
    expect(tauriProjectMocks.projectUpdate).toHaveBeenCalledWith('project-1', { name: 'Updated' });
    expect(tauriProjectMocks.projectDelete).toHaveBeenCalledWith('project-1', 'moveToInbox');
    expect(tauriProjectMocks.projectReorder).toHaveBeenCalledWith(['project-1']);
    expect(tauriProjectMocks.projectGetActiveId).toHaveBeenCalledTimes(1);
    expect(tauriProjectMocks.projectSetActiveId).toHaveBeenCalledWith(null);
  });
});
