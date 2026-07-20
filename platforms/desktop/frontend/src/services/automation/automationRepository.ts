import {
  loadAutomationRepositoryState as loadAutomationRepositoryStateFromService,
  saveAutomationProcessedEntries,
  saveAutomationProfiles,
  saveAutomationRepositoryState,
  saveAutomationRules,
  validateAutomationRuleForActivation,
} from '../automation/automationService';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import type { AutomationProcessedEntry, AutomationProfile, AutomationRule } from '../../types/automation';

export async function loadAutomationRepositoryState(): Promise<{
  profiles: AutomationProfile[];
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}> {
  return loadAutomationRepositoryStateFromService();
}

export async function persistAutomationProfiles(nextProfiles: AutomationProfile[]): Promise<void> {
  await saveAutomationProfiles(nextProfiles);
}

export async function persistAutomationRules(nextRules: AutomationRule[]): Promise<void> {
  await saveAutomationRules(nextRules);
}

export async function persistAutomationProcessedEntries(
  nextProcessedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await saveAutomationProcessedEntries(nextProcessedEntries);
}

export async function persistAutomationRepositoryState(
  nextProfiles: AutomationProfile[],
  nextRules: AutomationRule[],
  nextProcessedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await saveAutomationRepositoryState(nextProfiles, nextRules, nextProcessedEntries);
}

export async function validateAutomationRuleActivation(rule: AutomationRule): Promise<void> {
  const tagIds = rule.tagIds ?? (
    rule.projectId && rule.projectId !== 'inbox' && rule.projectId !== 'none' ? [rule.projectId] : []
  );
  const tags = tagIds
    .map((tagId) => useProjectStore.getState().getProjectById(tagId))
    .filter((tag): tag is NonNullable<typeof tag> => !!tag);
  const validation = await validateAutomationRuleForActivation(
    rule,
    useConfigStore.getState().config,
    tags,
  );

  if (!validation.valid) {
    throw new Error(validation.message || 'Automation rule validation failed.');
  }
}
