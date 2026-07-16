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

export interface AutomationServicePorts {
  automationLoadRepositoryState: typeof automationLoadRepositoryState;
  automationPersistProcessedEntries: typeof automationPersistProcessedEntries;
  automationPersistRepositoryState: typeof automationPersistRepositoryState;
  automationPersistRules: typeof automationPersistRules;
  automationValidateRuleActivation: typeof automationValidateRuleActivation;
}

export class AutomationService {
  constructor(private readonly ports: AutomationServicePorts) {}

  ensureAutomationStorage = async (): Promise<void> => {
    await this.ports.automationLoadRepositoryState();
  }

  loadAutomationRepositoryState = async (): Promise<AutomationRepositoryState> => {
    return this.ports.automationLoadRepositoryState();
  }

  loadAutomationRules = async (): Promise<AutomationRule[]> => {
    return (await this.ports.automationLoadRepositoryState()).rules;
  }

  saveAutomationRules = async (rules: AutomationRule[]): Promise<void> => {
    await this.ports.automationPersistRules(rules);
  }

  loadAutomationProcessedEntries = async (): Promise<AutomationProcessedEntry[]> => {
    return (await this.ports.automationLoadRepositoryState()).processedEntries;
  }

  saveAutomationProcessedEntries = async (entries: AutomationProcessedEntry[]): Promise<void> => {
    await this.ports.automationPersistProcessedEntries(entries);
  }

  saveAutomationRepositoryState = async (
    rules: AutomationRule[],
    processedEntries: AutomationProcessedEntry[],
  ): Promise<void> => {
    await this.ports.automationPersistRepositoryState(rules, processedEntries);
  }

  normalizeAutomationPath = (path: string): string => {
    return path.trim().replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
  }

  isSameAutomationPath = (a: string, b: string): boolean => {
    return this.normalizeAutomationPath(a) === this.normalizeAutomationPath(b);
  }

  isPathInsideDirectory = (filePath: string, directoryPath: string): boolean => {
    const normalizedFile = this.normalizeAutomationPath(filePath);
    const normalizedDirectory = this.normalizeAutomationPath(directoryPath);

    return normalizedFile === normalizedDirectory || normalizedFile.startsWith(`${normalizedDirectory}\\`);
  }

  createAutomationFingerprint = (filePath: string, size: number, mtimeMs: number): string => {
    return `${this.normalizeAutomationPath(filePath)}::${size}::${mtimeMs}`;
  }

  validateAutomationRuleForActivation = async (
    rule: AutomationRule,
    globalConfig: AppConfig,
    tags: ProjectRecord[],
  ): Promise<AutomationRuleValidationResult> => {
    return this.ports.automationValidateRuleActivation(rule, globalConfig, tags);
  }
}

export function createAutomationService(ports: AutomationServicePorts): AutomationService {
  return new AutomationService(ports);
}

export const automationService = createAutomationService({
  automationLoadRepositoryState,
  automationPersistProcessedEntries,
  automationPersistRepositoryState,
  automationPersistRules,
  automationValidateRuleActivation,
});

export const {
  ensureAutomationStorage,
  loadAutomationRepositoryState,
  loadAutomationRules,
  saveAutomationRules,
  loadAutomationProcessedEntries,
  saveAutomationProcessedEntries,
  saveAutomationRepositoryState,
  normalizeAutomationPath,
  isSameAutomationPath,
  isPathInsideDirectory,
  createAutomationFingerprint,
  validateAutomationRuleForActivation,
} = automationService;
