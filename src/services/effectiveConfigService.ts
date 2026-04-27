import type { AppConfig } from '../types/config';
import type { ProjectRecord } from '../types/project';
import {
  resolveProjectAwareHotwordSets,
  resolveProjectAwarePolishKeywordSets,
  resolveProjectAwareSpeakerProfiles,
  resolveProjectAwareTextReplacementSets,
} from '../types/project';
import { coerceSummaryTemplateId } from '../utils/summaryTemplates';

export function resolveEffectiveConfig(
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): AppConfig {
  const normalizedGlobalSummaryTemplateId = coerceSummaryTemplateId(
    globalConfig.summaryTemplateId,
    globalConfig.summaryCustomTemplates,
  );

  if (!project) {
    return {
      ...globalConfig,
      summaryTemplateId: normalizedGlobalSummaryTemplateId,
    };
  }

  return {
    ...globalConfig,
    summaryTemplateId: coerceSummaryTemplateId(
      project.defaults.summaryTemplateId,
      globalConfig.summaryCustomTemplates,
    ),
    translationLanguage: project.defaults.translationLanguage || globalConfig.translationLanguage,
    polishPresetId: project.defaults.polishPresetId || globalConfig.polishPresetId,
    textReplacementSets: resolveProjectAwareTextReplacementSets(globalConfig.textReplacementSets, project),
    hotwordSets: resolveProjectAwareHotwordSets(globalConfig.hotwordSets, project),
    polishKeywordSets: resolveProjectAwarePolishKeywordSets(globalConfig.polishKeywordSets, project),
    speakerProfiles: resolveProjectAwareSpeakerProfiles(globalConfig.speakerProfiles, project),
  };
}
