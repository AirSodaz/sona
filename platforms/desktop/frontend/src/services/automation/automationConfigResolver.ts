import type { AppConfig } from '../../types/config';
import type {
  AutomationActions,
  AutomationProfile,
  AutomationResolutionSnapshot,
  AutomationRule,
  AutomationStageConfig,
} from '../../types/automation';

const NO_ACTIONS: AutomationActions = {
  autoPolish: false,
  autoTranslate: false,
  autoSummary: false,
};

export function resolveMatchingTagAutomationRule(
  rules: AutomationRule[],
  tagIds: string[],
): AutomationRule | undefined {
  const tagIdSet = new Set(tagIds);
  return rules
    .filter((rule) => (
      rule.enabled
      && rule.kind === 'tag'
      && (rule.tagIds || []).some((tagId) => tagIdSet.has(tagId))
    ))
    .sort((left, right) => {
      const priorityDifference = (right.priority ?? 0) - (left.priority ?? 0);
      return priorityDifference || left.id.localeCompare(right.id);
    })[0];
}

export function applyAutomationProfile(
  globalConfig: AppConfig,
  profile?: AutomationProfile,
): AppConfig {
  if (!profile) return { ...globalConfig };

  const enabledByProfile = <T extends { id: string; enabled?: boolean }>(
    values: T[] | undefined,
    enabledIds: string[],
  ): T[] => {
    const enabled = new Set(enabledIds);
    return (values || []).map((value) => ({ ...value, enabled: enabled.has(value.id) }));
  };

  return {
    ...globalConfig,
    translationLanguage: profile.translationLanguage,
    polishPresetId: profile.polishPresetId,
    summaryTemplateId: profile.summaryTemplateId,
    textReplacementSets: enabledByProfile(globalConfig.textReplacementSets, profile.enabledTextReplacementSetIds),
    hotwordSets: enabledByProfile(globalConfig.hotwordSets, profile.enabledHotwordSetIds),
    polishKeywordSets: enabledByProfile(globalConfig.polishKeywordSets, profile.enabledPolishKeywordSetIds),
    speakerProfiles: enabledByProfile(globalConfig.speakerProfiles, profile.enabledSpeakerProfileIds),
  };
}

export function resolveAutomationQueueSnapshot(args: {
  globalConfig: AppConfig;
  profiles: AutomationProfile[];
  rules: AutomationRule[];
  fileRule?: AutomationRule;
  tagIds: string[];
  resolvedAt?: number;
}): {
  config: AppConfig;
  stageConfig: AutomationStageConfig;
  resolution: AutomationResolutionSnapshot;
} {
  const tagRule = resolveMatchingTagAutomationRule(args.rules, args.tagIds);
  const explicitFileProfile = args.fileRule?.profileSource === 'explicit' && args.fileRule.profileId
    ? args.profiles.find((profile) => profile.id === args.fileRule?.profileId)
    : undefined;
  const tagProfile = !explicitFileProfile && tagRule?.profileId
    ? args.profiles.find((profile) => profile.id === tagRule.profileId)
    : undefined;
  const profile = explicitFileProfile || tagProfile;
  const actions = tagRule?.actions ? { ...tagRule.actions } : { ...NO_ACTIONS };
  const config = applyAutomationProfile(args.globalConfig, profile);

  return {
    config,
    stageConfig: {
      autoPolish: actions.autoPolish,
      polishPresetId: config.polishPresetId || 'general',
      autoTranslate: actions.autoTranslate,
      translationLanguage: config.translationLanguage || 'en',
      autoSummary: actions.autoSummary,
      exportEnabled: args.fileRule?.stageConfig.exportEnabled ?? false,
    },
    resolution: {
      fileRuleId: args.fileRule?.id,
      tagRuleId: tagRule?.id,
      profileId: profile?.id,
      profileSource: explicitFileProfile ? 'file' : tagProfile ? 'tag' : 'global',
      actions,
      resolvedAt: args.resolvedAt ?? Date.now(),
    },
  };
}
