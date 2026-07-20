import type { ProjectCreateInput, ProjectRecord, ProjectUpdateInput } from '../types/project';
import {
  tagCreate as projectCreate,
  tagDelete as projectDelete,
  tagGetActiveId as projectGetActiveId,
  tagList as projectList,
  tagReorder as projectReorder,
  tagSaveAll as projectSaveAll,
  tagSetActiveId as projectSetActiveId,
  tagUpdate as projectUpdate,
} from './tauri/tag';

export interface ProjectServicePorts {
  projectCreate: typeof projectCreate;
  projectDelete: typeof projectDelete;
  projectGetActiveId: typeof projectGetActiveId;
  projectList: typeof projectList;
  projectReorder: typeof projectReorder;
  projectSaveAll: typeof projectSaveAll;
  projectSetActiveId: typeof projectSetActiveId;
  projectUpdate: typeof projectUpdate;
}

export class ProjectService {
  constructor(private readonly ports: ProjectServicePorts) {}

  async init(): Promise<void> {
    await this.ports.projectList();
  }

  async getAll(): Promise<ProjectRecord[]> {
    return this.ports.projectList();
  }

  async saveAll(projects: ProjectRecord[]): Promise<void> {
    await this.ports.projectSaveAll(projects);
  }

  async reorder(projectIds: string[]): Promise<void> {
    await this.ports.projectReorder(projectIds);
  }

  async create(input: ProjectCreateInput): Promise<ProjectRecord> {
    return this.ports.projectCreate(input);
  }

  async update(
    id: string,
    updates: ProjectUpdateInput,
  ): Promise<ProjectRecord | null> {
    return this.ports.projectUpdate(id, updates);
  }

  async delete(id: string): Promise<void> {
    await this.ports.projectDelete(id);
  }

  async getActiveProjectId(): Promise<string | null> {
    return this.ports.projectGetActiveId();
  }

  async setActiveProjectId(projectId: string | null): Promise<void> {
    await this.ports.projectSetActiveId(projectId);
  }
}

export function createProjectService(ports: ProjectServicePorts): ProjectService {
  return new ProjectService(ports);
}

export const projectService = createProjectService({
  projectCreate,
  projectDelete,
  projectGetActiveId,
  projectList,
  projectReorder,
  projectSaveAll,
  projectSetActiveId,
  projectUpdate,
});
