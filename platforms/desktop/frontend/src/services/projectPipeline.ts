import type { AppConfig } from '../types/config';
import type { ProjectPipelineConfig, ProjectRecord } from '../types/project';

export type EffectivePipelineSnapshot = ProjectPipelineConfig & {
  isProjectPipeline: boolean;
};

function globalDefaults(config: AppConfig): ProjectPipelineConfig {
  return {
    enabled: false,
    autoPolish: config.autoPolish ?? false,
    polishPresetId: config.polishPresetId,
    autoTranslate: false,
    targetLanguage: config.translationLanguage,
    autoSummary: config.summaryEnabled ?? false,
    summaryTemplateId: config.summaryTemplateId,
    autoExport: false,
  };
}

export function resolveItemPipeline(
  projectId: string | null | undefined,
  projects: ProjectRecord[] = [],
  globalConfig: AppConfig,
): EffectivePipelineSnapshot {
  const defaults = globalDefaults(globalConfig);
  const project = projectId ? (projects ?? []).find((candidate) => candidate.id === projectId) : undefined;
  if (!project?.pipeline?.enabled) {
    return { ...defaults, isProjectPipeline: false };
  }
  return {
    ...defaults,
    ...project.pipeline,
    polishPresetId: project.pipeline.polishPresetId ?? defaults.polishPresetId,
    targetLanguage: project.pipeline.targetLanguage ?? defaults.targetLanguage,
    summaryTemplateId: project.pipeline.summaryTemplateId ?? defaults.summaryTemplateId,
    isProjectPipeline: true,
  };
}
