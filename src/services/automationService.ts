import { BaseDirectory, exists, mkdir, readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import type { AppConfig } from '../types/config';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
} from '../types/automation';
import type { ProjectRecord } from '../types/project';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/runtime';
import { logger } from '../utils/logger';

const AUTOMATION_DIR = 'automation';
const RULES_FILE = `${AUTOMATION_DIR}/rules.json`;
const PROCESSED_FILE = `${AUTOMATION_DIR}/processed.json`;

export async function ensureAutomationStorage(): Promise<void> {
  const automationDirExists = await exists(AUTOMATION_DIR, { baseDir: BaseDirectory.AppLocalData });
  if (!automationDirExists) {
    await mkdir(AUTOMATION_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
  }

  const rulesExists = await exists(RULES_FILE, { baseDir: BaseDirectory.AppLocalData });
  if (!rulesExists) {
    await writeTextFile(RULES_FILE, '[]', { baseDir: BaseDirectory.AppLocalData });
  }

  const processedExists = await exists(PROCESSED_FILE, { baseDir: BaseDirectory.AppLocalData });
  if (!processedExists) {
    await writeTextFile(PROCESSED_FILE, '[]', { baseDir: BaseDirectory.AppLocalData });
  }
}

export async function loadAutomationRules(): Promise<AutomationRule[]> {
  try {
    await ensureAutomationStorage();
    const content = await readTextFile(RULES_FILE, { baseDir: BaseDirectory.AppLocalData });
    const parsed = JSON.parse(content) as AutomationRule[];
    return Array.isArray(parsed) ? parsed : [];
  } catch (error) {
    logger.error('[Automation] Failed to load rules:', error);
    return [];
  }
}

export async function saveAutomationRules(rules: AutomationRule[]): Promise<void> {
  await ensureAutomationStorage();
  await writeTextFile(RULES_FILE, JSON.stringify(rules, null, 2), {
    baseDir: BaseDirectory.AppLocalData,
  });
}

export async function loadAutomationProcessedEntries(): Promise<AutomationProcessedEntry[]> {
  try {
    await ensureAutomationStorage();
    const content = await readTextFile(PROCESSED_FILE, { baseDir: BaseDirectory.AppLocalData });
    const parsed = JSON.parse(content) as AutomationProcessedEntry[];
    return Array.isArray(parsed) ? parsed : [];
  } catch (error) {
    logger.error('[Automation] Failed to load processed entries:', error);
    return [];
  }
}

export async function saveAutomationProcessedEntries(entries: AutomationProcessedEntry[]): Promise<void> {
  await ensureAutomationStorage();
  await writeTextFile(PROCESSED_FILE, JSON.stringify(entries, null, 2), {
    baseDir: BaseDirectory.AppLocalData,
  });
}

export async function ensureDirectoryExists(directoryPath: string): Promise<void> {
  if (await exists(directoryPath)) {
    return;
  }

  await mkdir(directoryPath, { recursive: true });
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
  if (!rule.name.trim()) {
    return { valid: false, code: 'automation.name_required', message: 'Rule name is required.' };
  }

  if (!project) {
    return { valid: false, code: 'automation.project_missing', message: 'Select a target project.' };
  }

  if (!rule.watchDirectory.trim()) {
    return { valid: false, code: 'automation.watch_directory_required', message: 'Choose a watch directory.' };
  }

  if (!rule.exportConfig.directory.trim()) {
    return { valid: false, code: 'automation.output_directory_required', message: 'Choose an output directory.' };
  }

  if (isSameAutomationPath(rule.watchDirectory, rule.exportConfig.directory)) {
    return { valid: false, code: 'automation.same_directory', message: 'Watch and output directories must be different.' };
  }

  if (!(await exists(rule.watchDirectory))) {
    return { valid: false, code: 'automation.watch_directory_missing', message: 'The watch directory does not exist.' };
  }

  try {
    await ensureDirectoryExists(rule.exportConfig.directory);
  } catch (error) {
    logger.error('[Automation] Failed to prepare output directory:', error);
    return { valid: false, code: 'automation.output_directory_invalid', message: 'The output directory could not be created.' };
  }

  if (!globalConfig.offlineModelPath.trim() || !(await exists(globalConfig.offlineModelPath))) {
    return { valid: false, code: 'automation.offline_model_missing', message: 'An offline model is required before automation can run.' };
  }

  if (rule.stageConfig.autoPolish && !isLlmConfigComplete(getFeatureLlmConfig(globalConfig, 'polish'))) {
    return { valid: false, code: 'automation.polish_model_missing', message: 'A polish model is required for Auto-Polish.' };
  }

  if (rule.stageConfig.autoTranslate && !isLlmConfigComplete(getFeatureLlmConfig(globalConfig, 'translation'))) {
    return { valid: false, code: 'automation.translation_model_missing', message: 'A translation model is required for Auto-Translate.' };
  }

  if (
    (rule.exportConfig.mode === 'translation' || rule.exportConfig.mode === 'bilingual')
    && !rule.stageConfig.autoTranslate
  ) {
    return { valid: false, code: 'automation.translation_required', message: 'Enable Auto-Translate before exporting translations.' };
  }

  return { valid: true };
}
