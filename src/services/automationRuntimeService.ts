import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AutomationRule } from '../types/automation';

export const AUTOMATION_RUNTIME_CANDIDATE_EVENT = 'automation-runtime-candidate';
export const DEFAULT_AUTOMATION_CANDIDATE_DEBOUNCE_MS = 250;
export const DEFAULT_AUTOMATION_STABLE_WINDOW_MS = 5000;

export interface AutomationRuntimeRuleConfig {
  ruleId: string;
  watchDirectory: string;
  recursive: boolean;
  excludeDirectory: string;
  debounceMs: number;
  stableWindowMs: number;
}

export interface AutomationRuntimeReplaceResult {
  ruleId: string;
  started: boolean;
  error?: string | null;
}

export interface AutomationRuntimeCandidatePayload {
  ruleId: string;
  filePath: string;
  sourceFingerprint: string;
  size: number;
  mtimeMs: number;
}

export type AutomationRuntimePathCollectionOutcome =
  | 'candidate'
  | 'missing'
  | 'unsupported'
  | 'excluded'
  | 'not_file'
  | 'error';

export interface AutomationRuntimePathCollectionResult {
  filePath: string;
  outcome: AutomationRuntimePathCollectionOutcome;
  candidate?: AutomationRuntimeCandidatePayload | null;
  error?: string | null;
}

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
  return invoke<AutomationRuntimeReplaceResult[]>('replace_automation_runtime_rules', {
    rules,
  });
}

export async function scanAutomationRuntimeRule(
  rule: AutomationRuntimeRuleConfig,
): Promise<void> {
  await invoke('scan_automation_runtime_rule', {
    rule,
  });
}

export async function collectAutomationRuntimeRulePaths(
  rule: AutomationRuntimeRuleConfig,
  filePaths: string[],
): Promise<AutomationRuntimePathCollectionResult[]> {
  return invoke<AutomationRuntimePathCollectionResult[]>('collect_automation_runtime_rule_paths', {
    rule,
    filePaths,
  });
}

export async function listenToAutomationRuntimeCandidates(
  onCandidate: (payload: AutomationRuntimeCandidatePayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<AutomationRuntimeCandidatePayload>(AUTOMATION_RUNTIME_CANDIDATE_EVENT, ({ payload }) => {
    void onCandidate(payload);
  });
}
