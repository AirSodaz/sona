import type {
  ProjectCreateInput,
  ProjectPipelineConfig,
  ProjectRecord,
  ProjectUpdateInput,
} from '../types/project';
import {
  projectCreate,
  projectDelete,
  projectGetActiveId,
  projectList,
  projectReorder,
  projectSetActiveId,
  projectUpdate,
} from './tauri/project';

export interface ProjectServicePorts {
  projectCreate: typeof projectCreate;
  projectDelete: typeof projectDelete;
  projectGetActiveId: typeof projectGetActiveId;
  projectList: typeof projectList;
  projectReorder: typeof projectReorder;
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
    await Promise.all(
      projects.map((project) =>
        this.ports.projectUpdate(project.id, {
          name: project.name,
          description: project.description,
          icon: project.icon,
          color: project.color,
          pipeline: project.pipeline,
        })
      )
    );
  }

  async reorder(projectIds: string[]): Promise<void> {
    await this.ports.projectReorder(projectIds);
  }

  async create(input: ProjectCreateInput): Promise<ProjectRecord> {
    return this.ports.projectCreate(input);
  }

  async update(id: string, updates: ProjectUpdateInput): Promise<ProjectRecord | null> {
    return this.ports.projectUpdate(id, updates);
  }

  async updatePipeline(id: string, pipeline: ProjectPipelineConfig): Promise<ProjectRecord | null> {
    return this.update(id, { pipeline });
  }

  async delete(
    id: string,
    cascadeAction: 'moveToInbox' | 'deleteItems' = 'moveToInbox'
  ): Promise<void> {
    await this.ports.projectDelete(id, cascadeAction);
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
  projectSetActiveId,
  projectUpdate,
});
