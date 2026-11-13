import type { AutomationProcessedRecord_Serialize, AutomationRuleRecord } from '../../bindings';
import type {
  AutomationExportConfig,
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
  AutomationStageConfig,
} from '../../types/automation';
import type { AppConfig } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import type { ExportFormat, ExportMode } from '../../utils/exportFormats';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface AutomationRepositoryState {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}

const EXPORT_FORMATS = new Set(['srt', 'json', 'txt', 'vtt', 'md']);
const EXPORT_MODES = new Set(['original', 'translation', 'bilingual']);
const PROCESSED_STATUSES = new Set(['pending', 'complete', 'error', 'discarded']);

function normalizeAutomationRule(record: AutomationRuleRecord): AutomationRule {
  const exportConfig: AutomationExportConfig = {
    directory: record.exportConfig.directory || '',
    format: (EXPORT_FORMATS.has(record.exportConfig.format)
      ? record.exportConfig.format
      : 'txt') as ExportFormat,
    mode: (EXPORT_MODES.has(record.exportConfig.mode)
      ? record.exportConfig.mode
      : 'original') as ExportMode,
    prefix: record.exportConfig.prefix || '',
  };

  const stageConfig: AutomationStageConfig = {
    autoPolish: Boolean(record.stageConfig.autoPolish),
    polishPresetId: record.stageConfig.polishPresetId || 'general',
    autoTranslate: Boolean(record.stageConfig.autoTranslate),
    translationLanguage: record.stageConfig.translationLanguage || 'en',
  };

  return {
    id: record.id,
    name: record.name,
    saveHistory: record.saveHistory,
    projectId: record.projectId || '',
    watchDirectory: record.watchDirectory,
    recursive: record.recursive,
    enabled: record.enabled,
    stageConfig,
    exportConfig,
    createdAt: record.createdAt,
    updatedAt: record.updatedAt,
  };
}

function normalizeAutomationProcessedEntry(
  record: AutomationProcessedRecord_Serialize
): AutomationProcessedEntry {
  return {
    id: record.id,
    ruleId: record.ruleId,
    attempt: Number(record.attempt || 1),
    filePath: record.filePath,
    sourceFingerprint: record.sourceFingerprint,
    size: record.size,
    mtimeMs: record.mtimeMs,
    status: PROCESSED_STATUSES.has(record.status)
      ? (record.status as AutomationProcessedEntry['status'])
      : 'error',
    processedAt: record.processedAt,
    historyId: record.historyId ?? undefined,
    exportPath: record.exportPath ?? undefined,
    errorMessage: record.errorMessage ?? undefined,
  };
}

export async function automationLoadRepositoryState(): Promise<AutomationRepositoryState> {
  const state = await invokeTauri(TauriCommand.automationRepository.loadState);
  return {
    rules: state.rules.map(normalizeAutomationRule),
    processedEntries: state.processedEntries.map(normalizeAutomationProcessedEntry),
  };
}

function toAutomationRuleInput(rule: AutomationRule) {
  return {
    id: rule.id,
    name: rule.name,
    saveHistory: rule.saveHistory,
    projectId: rule.projectId,
    watchDirectory: rule.watchDirectory,
    recursive: rule.recursive,
    enabled: rule.enabled,
    stageConfig: {
      autoPolish: rule.stageConfig.autoPolish,
      polishPresetId: rule.stageConfig.polishPresetId || 'general',
      autoTranslate: rule.stageConfig.autoTranslate,
      translationLanguage: rule.stageConfig.translationLanguage || 'en',
    },
    exportConfig: {
      directory: rule.exportConfig.directory,
      format: rule.exportConfig.format,
      mode: rule.exportConfig.mode,
      prefix: rule.exportConfig.prefix,
    },
    createdAt: rule.createdAt,
    updatedAt: rule.updatedAt,
  };
}

export async function automationPersistRules(rules: AutomationRule[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistRules, {
    rules: rules.map(toAutomationRuleInput),
  });
}

export async function automationPersistProcessedEntries(
  processedEntries: AutomationProcessedEntry[]
): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistProcessedEntries, {
    processedEntries,
  });
}

export async function automationPersistRepositoryState(
  rules: AutomationRule[],
  processedEntries: AutomationProcessedEntry[]
): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistState, {
    rules: rules.map(toAutomationRuleInput),
    processedEntries,
  });
}

export async function automationValidateRuleActivation(
  rule: AutomationRule,
  globalConfig: AppConfig,
  projects: ProjectRecord[] = []
): Promise<AutomationRuleValidationResult> {
  return invokeTauri(TauriCommand.automationRepository.validateActivation, {
    rule: {
      name: rule.name,
      saveHistory: rule.saveHistory,
      projectId: rule.projectId,
      watchDirectory: rule.watchDirectory,
      recursive: rule.recursive,
      stageConfig: {
        autoPolish: rule.stageConfig.autoPolish,
        autoTranslate: rule.stageConfig.autoTranslate,
      },
      exportConfig: {
        directory: rule.exportConfig.directory,
        mode: rule.exportConfig.mode,
      },
    },
    globalConfig,
    projects,
  });
}
