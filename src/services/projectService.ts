import type { ProjectDefaults, ProjectRecord } from '../types/project';
import {
  projectCreate,
  projectDelete,
  projectGetActiveId,
  projectList,
  projectReorder,
  projectSaveAll,
  projectSetActiveId,
  projectUpdate,
} from './tauri/project';

export const projectService = {
  async init(): Promise<void> {
    await projectList();
  },

  async getAll(options?: {
    fallbackEnabledPolishKeywordSetIds?: string[];
    fallbackEnabledSpeakerProfileIds?: string[];
  }): Promise<ProjectRecord[]> {
    return projectList(options);
  },

  async saveAll(projects: ProjectRecord[]): Promise<void> {
    await projectSaveAll(projects);
  },

  async reorder(projectIds: string[]): Promise<void> {
    await projectReorder(projectIds);
  },

  async create(input: {
    name: string;
    description?: string;
    icon?: string;
    defaults: ProjectDefaults;
  }): Promise<ProjectRecord> {
    return projectCreate(input);
  },

  async update(
    id: string,
    updates: Partial<Pick<ProjectRecord, 'name' | 'description' | 'icon' | 'defaults'>>,
  ): Promise<ProjectRecord | null> {
    return projectUpdate(id, updates);
  },

  async delete(id: string): Promise<void> {
    await projectDelete(id);
  },

  async getActiveProjectId(): Promise<string | null> {
    return projectGetActiveId();
  },

  async setActiveProjectId(projectId: string | null): Promise<void> {
    await projectSetActiveId(projectId);
  },
};
