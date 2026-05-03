import type { ProjectRecord } from '../../types/project';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

export type ProjectListRequest = TauriCommandArgs<typeof TauriCommand.project.list>;
export type ProjectCreateRequest = TauriCommandArgs<typeof TauriCommand.project.create>;
export type ProjectUpdateRequest = TauriCommandArgs<typeof TauriCommand.project.update>['updates'];

export async function projectList(
  request: ProjectListRequest = {},
): Promise<ProjectRecord[]> {
  return invokeTauri(TauriCommand.project.list, request);
}

export async function projectSaveAll(projects: ProjectRecord[]): Promise<void> {
  await invokeTauri(TauriCommand.project.saveAll, { projects });
}

export async function projectCreate(
  request: ProjectCreateRequest,
): Promise<ProjectRecord> {
  return invokeTauri(TauriCommand.project.create, request);
}

export async function projectUpdate(
  projectId: string,
  updates: ProjectUpdateRequest,
): Promise<ProjectRecord | null> {
  return invokeTauri(TauriCommand.project.update, { projectId, updates });
}

export async function projectDelete(projectId: string): Promise<void> {
  await invokeTauri(TauriCommand.project.delete, { projectId });
}

export async function projectReorder(projectIds: string[]): Promise<ProjectRecord[]> {
  return invokeTauri(TauriCommand.project.reorder, { projectIds });
}

export async function projectGetActiveId(): Promise<string | null> {
  return invokeTauri(TauriCommand.project.getActiveId);
}

export async function projectSetActiveId(projectId: string | null): Promise<void> {
  await invokeTauri(TauriCommand.project.setActiveId, { projectId });
}
