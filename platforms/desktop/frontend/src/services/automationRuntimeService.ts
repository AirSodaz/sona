import type { AutomationRule } from '../types/automation';
import type {
  AutomationRuntimeCandidatePayload,
  AutomationRuntimePathCollectionResult,
  AutomationRuntimeReplaceResult,
  AutomationRuntimeRuleConfig,
} from '../bindings';
export type {
  AutomationRuntimeCandidatePayload,
  AutomationRuntimePathCollectionOutcome,
  AutomationRuntimePathCollectionResult,
  AutomationRuntimeReplaceResult,
  AutomationRuntimeRuleConfig,
} from '../bindings';
import {
  collectAutomationRuntimeRulePaths as collectAutomationRuntimeRulePathsTauri,
  replaceAutomationRuntimeRules as replaceAutomationRuntimeRulesTauri,
  scanAutomationRuntimeRule as scanAutomationRuntimeRuleTauri,
} from './tauri/automation';
import { TauriEvent } from './tauri/events';
import { listen, type UnlistenFn } from './tauri/platform/events';

export const AUTOMATION_RUNTIME_CANDIDATE_EVENT = TauriEvent.automation.runtimeCandidate;
export const DEFAULT_AUTOMATION_CANDIDATE_DEBOUNCE_MS = 250;
export const DEFAULT_AUTOMATION_STABLE_WINDOW_MS = 5000;

export function toAutomationRuntimeRuleConfig(
  rule: Pick<AutomationRule, 'id' | 'watchDirectory' | 'recursive' | 'exportConfig'>,
): AutomationRuntimeRuleConfig {
  return {
    ruleId: rule.id,
    watchDirectory: rule.watchDirectory.trim(),
    recursive: rule.recursive,
    excludeDirectory: rule.exportConfig.directory.trim(),
    debounceMs: DEFAULT_AUTOMATION_CANDIDATE_DEBOUNCE_MS,
    stableWindowMs: DEFAULT_AUTOMATION_STABLE_WINDOW_MS,
  };
}

export async function replaceAutomationRuntimeRules(
  rules: AutomationRuntimeRuleConfig[],
): Promise<AutomationRuntimeReplaceResult[]> {
  return replaceAutomationRuntimeRulesTauri(rules);
}

export async function scanAutomationRuntimeRule(
  rule: AutomationRuntimeRuleConfig,
): Promise<void> {
  await scanAutomationRuntimeRuleTauri(rule);
}

export async function collectAutomationRuntimeRulePaths(
  rule: AutomationRuntimeRuleConfig,
  filePaths: string[],
): Promise<AutomationRuntimePathCollectionResult[]> {
  return collectAutomationRuntimeRulePathsTauri(rule, filePaths);
}

export async function listenToAutomationRuntimeCandidates(
  onCandidate: (payload: AutomationRuntimeCandidatePayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<AutomationRuntimeCandidatePayload>(AUTOMATION_RUNTIME_CANDIDATE_EVENT, ({ payload }) => {
    void onCandidate(payload);
  });
}
