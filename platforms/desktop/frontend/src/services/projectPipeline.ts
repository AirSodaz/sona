import type { AppConfig } from '../types/config';
import type {
  EffectivePipelineSnapshot,
  ProjectPipelineConfig,
  ProjectRecord,
} from '../types/project';

export type { EffectivePipelineSnapshot };

function globalDefaults(config: AppConfig): ProjectPipelineConfig {
  return {
    enabled: false,
    autoPolish: config.autoPolish ?? false,
    polishPresetId: config.polishPresetId,
    autoTranslate: config.autoTranslate ?? false,
    targetLanguage: config.translationLanguage,
    autoSummary: config.autoSummary ?? false,
    summaryTemplateId: config.summaryTemplateId,
    autoExport: false,
    customTerms: [],
  };
}

export function resolveItemPipeline(
  projectId: string | null | undefined,
  projects: ProjectRecord[] = [],
  globalConfig: AppConfig
): EffectivePipelineSnapshot {
  const defaults = globalDefaults(globalConfig);
  const project = projectId
    ? (projects ?? []).find((candidate) => candidate.id === projectId)
    : undefined;
  if (!project?.pipeline?.enabled) {
    return { ...defaults, isProjectPipeline: false };
  }
  return {
    ...defaults,
    ...project.pipeline,
    polishPresetId: project.pipeline.polishPresetId ?? defaults.polishPresetId,
    targetLanguage: project.pipeline.targetLanguage ?? defaults.targetLanguage,
    summaryTemplateId: project.pipeline.summaryTemplateId ?? defaults.summaryTemplateId,
    customTerms: project.pipeline.customTerms ?? defaults.customTerms ?? [],
    isProjectPipeline: true,
  };
}

export function resolveHistoryItemPipeline(
  historyId: string | null | undefined,
  historyItems: Array<{ id: string; projectId?: string | null }> = [],
  projects: ProjectRecord[] = [],
  globalConfig: AppConfig
): EffectivePipelineSnapshot {
  const item = historyId ? historyItems.find((h) => h.id === historyId) : undefined;
  return resolveItemPipeline(item?.projectId, projects, globalConfig);
}
