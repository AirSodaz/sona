import type { ProjectRecord as GeneratedProjectRecord } from '../../bindings';
import {
  DEFAULT_PROJECT_PIPELINE,
  type ProjectCreateInput,
  type ProjectPipelineConfig,
  type ProjectRecord,
  type ProjectUpdateInput,
} from '../../types/project';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

export type ProjectListRequest = TauriCommandArgs<typeof TauriCommand.project.list>;

function normalizeProject(record: GeneratedProjectRecord): ProjectRecord {
  const pipeline: ProjectPipelineConfig | undefined = record.pipeline
    ? {
        ...DEFAULT_PROJECT_PIPELINE,
        enabled: record.pipeline.enabled ?? false,
        autoPolish: record.pipeline.autoPolish ?? false,
        polishPresetId: record.pipeline.polishPresetId ?? undefined,
        polishPromptOverride: record.pipeline.polishPromptOverride ?? undefined,
        autoTranslate: record.pipeline.autoTranslate ?? false,
        targetLanguage: record.pipeline.targetLanguage ?? undefined,
        autoSummary: record.pipeline.autoSummary ?? false,
        summaryTemplateId: record.pipeline.summaryTemplateId ?? undefined,
        hotwordSetIds: record.pipeline.hotwordSetIds ?? [],
        replacementSetIds: record.pipeline.replacementSetIds ?? [],
        customTerms: record.pipeline.customTerms ?? [],
        autoExport: record.pipeline.autoExport ?? false,
        exportFormat: record.pipeline.exportFormat as ProjectPipelineConfig['exportFormat'],
        exportDirectory: record.pipeline.exportDirectory ?? undefined,
        exportFileNamePrefix: record.pipeline.exportFileNamePrefix ?? undefined,
      }
    : undefined;
  return {
    id: record.id,
    name: record.name,
    description: record.description ?? '',
    icon: record.icon ?? '',
    color: record.color ?? undefined,
    sortOrder: record.sortOrder,
    createdAt: record.createdAt,
    updatedAt: record.updatedAt,
    pipeline,
  };
}

export async function projectList(_request?: ProjectListRequest): Promise<ProjectRecord[]> {
  void _request;
  const records = await invokeTauri(TauriCommand.project.list);
  return records.map(normalizeProject);
}

export async function projectCreate(input: ProjectCreateInput): Promise<ProjectRecord> {
  return normalizeProject(await invokeTauri(TauriCommand.project.create, { input }));
}

export async function projectUpdate(
  projectId: string,
  updates: ProjectUpdateInput
): Promise<ProjectRecord | null> {
  const record = await invokeTauri(TauriCommand.project.update, { projectId, updates });
  return record ? normalizeProject(record) : null;
}

export async function projectDelete(
  projectId: string,
  cascadeAction: 'moveToInbox' | 'deleteItems' = 'moveToInbox'
): Promise<void> {
  await invokeTauri(TauriCommand.project.delete, { projectId, cascadeAction });
}

export async function projectReorder(projectIds: string[]): Promise<ProjectRecord[]> {
  const records = await invokeTauri(TauriCommand.project.reorder, { projectIds });
  return records.map(normalizeProject);
}

export async function projectGetActiveId(): Promise<string | null> {
  return await invokeTauri(TauriCommand.project.getActiveId);
}

export async function projectSetActiveId(projectId: string | null): Promise<void> {
  await invokeTauri(TauriCommand.project.setActiveId, { projectId });
}
