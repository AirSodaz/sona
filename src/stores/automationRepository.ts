import {
  ensureAutomationStorage,
  loadAutomationProcessedEntries,
  loadAutomationRules,
  saveAutomationProcessedEntries,
  saveAutomationRules,
  validateAutomationRuleForActivation,
} from '../services/automationService';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import type { AutomationProcessedEntry, AutomationRule } from '../types/automation';

export async function loadAutomationRepositoryState(): Promise<{
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}> {
  await ensureAutomationStorage();
  const [rules, processedEntries] = await Promise.all([
    loadAutomationRules(),
    loadAutomationProcessedEntries(),
  ]);

  return { rules, processedEntries };
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
  nextRules: AutomationRule[],
  nextProcessedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await Promise.all([
    saveAutomationRules(nextRules),
    saveAutomationProcessedEntries(nextProcessedEntries),
  ]);
}

export async function validateAutomationRuleActivation(rule: AutomationRule): Promise<void> {
  const isInboxOrNone = rule.projectId === 'inbox' || rule.projectId === 'none';
  const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(rule.projectId);
  const validation = await validateAutomationRuleForActivation(
    rule,
    useConfigStore.getState().config,
    project,
  );

  if (!validation.valid) {
    throw new Error(validation.message || 'Automation rule validation failed.');
  }
}
