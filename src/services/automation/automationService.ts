import type { AppConfig } from '../../types/config';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
} from '../../types/automation';
import type { ProjectRecord } from '../../types/project';
import {
  automationLoadRepositoryState,
  automationPersistProcessedEntries,
  automationPersistRepositoryState,
  automationPersistRules,
  automationValidateRuleActivation,
  type AutomationRepositoryState,
} from '../tauri/automationRepository';

export async function ensureAutomationStorage(): Promise<void> {
  await automationLoadRepositoryState();
}

export async function loadAutomationRepositoryState(): Promise<AutomationRepositoryState> {
  return automationLoadRepositoryState();
}

export async function loadAutomationRules(): Promise<AutomationRule[]> {
  return (await automationLoadRepositoryState()).rules;
}

export async function saveAutomationRules(rules: AutomationRule[]): Promise<void> {
  await automationPersistRules(rules);
}

export async function loadAutomationProcessedEntries(): Promise<AutomationProcessedEntry[]> {
  return (await automationLoadRepositoryState()).processedEntries;
}

export async function saveAutomationProcessedEntries(entries: AutomationProcessedEntry[]): Promise<void> {
  await automationPersistProcessedEntries(entries);
}

export async function saveAutomationRepositoryState(
  rules: AutomationRule[],
  processedEntries: AutomationProcessedEntry[],
): Promise<void> {
  await automationPersistRepositoryState(rules, processedEntries);
}

export function normalizeAutomationPath(path: string): string {
  return path.trim().replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
}

export function isSameAutomationPath(a: string, b: string): boolean {
  return normalizeAutomationPath(a) === normalizeAutomationPath(b);
}

export function isPathInsideDirectory(filePath: string, directoryPath: string): boolean {
  const normalizedFile = normalizeAutomationPath(filePath);
  const normalizedDirectory = normalizeAutomationPath(directoryPath);

  return normalizedFile === normalizedDirectory || normalizedFile.startsWith(`${normalizedDirectory}\\`);
}

export function createAutomationFingerprint(filePath: string, size: number, mtimeMs: number): string {
  return `${normalizeAutomationPath(filePath)}::${size}::${mtimeMs}`;
}

export async function validateAutomationRuleForActivation(
  rule: AutomationRule,
  globalConfig: AppConfig,
  project: ProjectRecord | null,
): Promise<AutomationRuleValidationResult> {
  return automationValidateRuleActivation(rule, globalConfig, project);
}
