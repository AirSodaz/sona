import type { AppConfig } from '../types/config';
import type { ProjectRecord } from '../types/project';
import {
  resolveProjectAwareHotwordSets,
  resolveProjectAwareTextReplacementSets,
} from '../types/project';

export function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): AppConfig {
  if (!project) {
    return globalConfig;
  }

  return {
    ...globalConfig,
    translationLanguage: project.defaults.translationLanguage || globalConfig.translationLanguage,
    polishPresetId: project.defaults.polishPresetId || globalConfig.polishPresetId,
    textReplacementSets: resolveProjectAwareTextReplacementSets(globalConfig.textReplacementSets, project),
    hotwordSets: resolveProjectAwareHotwordSets(globalConfig.hotwordSets, project),
  };
}
