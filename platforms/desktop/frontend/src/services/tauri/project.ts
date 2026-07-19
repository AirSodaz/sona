import type {
  ProjectRecord_Deserialize,
  ProjectRecord_Serialize,
} from '../../bindings';
import type { ProjectRecord } from '../../types/project';
import { normalizeProjectRecord } from '../project/projectDefaults';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';
import { toTagDefaultsTransport } from './tagRecordTransport';

export type ProjectListRequest = TauriCommandArgs<typeof TauriCommand.project.list>;
export type ProjectCreateRequest = TauriCommandArgs<typeof TauriCommand.project.create>;
export type ProjectUpdateRequest = TauriCommandArgs<typeof TauriCommand.project.update>['updates'];

function toProjectRecordTransport(
  project: ProjectRecord,
): ProjectRecord_Deserialize {
  return {
    id: project.id,
    name: project.name,
    description: project.description,
    icon: project.icon,
    createdAt: project.createdAt,
    updatedAt: project.updatedAt,
    defaults: toTagDefaultsTransport(project.defaults),
  };
}

function normalizeProjectTransport(
  project: ProjectRecord_Serialize,
): ProjectRecord {
  return normalizeProjectRecord(project);
}

export async function projectList(
  request: ProjectListRequest = {},
): Promise<ProjectRecord[]> {
  const projects = await invokeTauri(TauriCommand.project.list, request);
  return projects.map(normalizeProjectTransport);
}

export async function projectSaveAll(projects: ProjectRecord[]): Promise<void> {
  await invokeTauri(TauriCommand.project.saveAll, {
    projects: projects.map(toProjectRecordTransport),
  });
}

export async function projectCreate(
  request: ProjectCreateRequest,
): Promise<ProjectRecord> {
  const project = await invokeTauri(TauriCommand.project.create, request);
  return normalizeProjectTransport(project);
}

export async function projectUpdate(
  projectId: string,
  updates: ProjectUpdateRequest,
): Promise<ProjectRecord | null> {
  const project = await invokeTauri(TauriCommand.project.update, { projectId, updates });
  return project ? normalizeProjectTransport(project) : null;
}

export async function projectDelete(projectId: string): Promise<void> {
  await invokeTauri(TauriCommand.project.delete, { projectId });
}

export async function projectReorder(projectIds: string[]): Promise<ProjectRecord[]> {
  const projects = await invokeTauri(TauriCommand.project.reorder, { projectIds });
  return projects.map(normalizeProjectTransport);
}

export async function projectGetActiveId(): Promise<string | null> {
  return invokeTauri(TauriCommand.project.getActiveId);
}

export async function projectSetActiveId(projectId: string | null): Promise<void> {
  await invokeTauri(TauriCommand.project.setActiveId, { projectId });
}
