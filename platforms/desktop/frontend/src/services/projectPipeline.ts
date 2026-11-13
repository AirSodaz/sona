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
    autoPolish: false,
    autoPolishFrequency: 5,
    polishPresetId: config.polishPresetId,
    autoTranslate: false,
    targetLanguage: config.translationLanguage,
    autoSummary: false,
    summaryTemplateId: config.summaryTemplateId,
    customTerms: [],
    autoExport: false,
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
  // Mirrors `core::project::resolve_item_pipeline`: a non-positive frequency means
  // "unset", never "polish on every segment".
  const frequency = project.pipeline.autoPolishFrequency ?? 0;
  return {
    ...defaults,
    ...project.pipeline,
    autoPolishFrequency: frequency > 0 ? frequency : 5,
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
