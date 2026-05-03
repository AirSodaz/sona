import type { AppConfig } from '../../types/config';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
} from '../../types/automation';
import type { ProjectRecord } from '../../types/project';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface AutomationRepositoryState {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}

export async function automationLoadRepositoryState(): Promise<AutomationRepositoryState> {
  return invokeTauri(TauriCommand.automationRepository.loadState);
}

export async function automationPersistRules(rules: AutomationRule[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistRules, { rules });
}

export async function automationPersistProcessedEntries(
  processedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistProcessedEntries, {
    processedEntries,
  });
}

export async function automationPersistRepositoryState(
  rules: AutomationRule[],
  processedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistState, {
    rules,
    processedEntries,
  });
}

export async function automationValidateRuleActivation(
  rule: AutomationRule,
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): Promise<AutomationRuleValidationResult> {
  return invokeTauri(TauriCommand.automationRepository.validateActivation, {
    rule,
    globalConfig,
    project,
  });
}
