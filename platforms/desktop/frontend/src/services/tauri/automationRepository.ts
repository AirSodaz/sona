import type { AppConfig } from '../../types/config';
import type {
  AutomationProcessedRecord_Serialize,
  AutomationRuleRecord,
} from '../../bindings';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuleValidationResult,
} from '../../types/automation';
import type { TagRecord } from '../../types/tag';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface AutomationRepositoryState {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}

const AUTOMATION_PRESET_IDS = new Set([
  'meeting_notes',
  'lecture_notes',
  'bilingual_subtitles',
  'custom',
]);
const EXPORT_FORMATS = new Set(['srt', 'json', 'txt', 'vtt', 'md']);
const EXPORT_MODES = new Set(['original', 'translation', 'bilingual']);
const PROCESSED_STATUSES = new Set(['complete', 'error', 'discarded']);

function normalizeAutomationRule(record: AutomationRuleRecord): AutomationRule {
  return {
    ...record,
    saveHistory: record.saveHistory,
    tagIds: record.tagIds,
    presetId: AUTOMATION_PRESET_IDS.has(record.presetId)
      ? record.presetId as AutomationRule['presetId']
      : 'custom',
    stageConfig: {
      ...record.stageConfig,
      polishPresetId: record.stageConfig.polishPresetId || undefined,
      translationLanguage: record.stageConfig.translationLanguage || undefined,
    },
    exportConfig: {
      ...record.exportConfig,
      format: EXPORT_FORMATS.has(record.exportConfig.format)
        ? record.exportConfig.format as AutomationRule['exportConfig']['format']
        : 'txt',
      mode: EXPORT_MODES.has(record.exportConfig.mode)
        ? record.exportConfig.mode as AutomationRule['exportConfig']['mode']
        : 'original',
      prefix: record.exportConfig.prefix || undefined,
    },
  };
}

function normalizeAutomationProcessedEntry(
  record: AutomationProcessedRecord_Serialize,
): AutomationProcessedEntry {
  return {
    id: record.id,
    ruleId: record.ruleId,
    filePath: record.filePath,
    sourceFingerprint: record.sourceFingerprint,
    size: record.size,
    mtimeMs: record.mtimeMs,
    status: PROCESSED_STATUSES.has(record.status)
      ? record.status as AutomationProcessedEntry['status']
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

export async function automationPersistRules(rules: AutomationRule[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistRules, {
    rules: rules.map(toAutomationRuleInput),
  });
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
    rules: rules.map(toAutomationRuleInput),
    processedEntries,
  });
}

export async function automationValidateRuleActivation(
  rule: AutomationRule,
  globalConfig: AppConfig,
  tags: TagRecord[] | TagRecord | null,
): Promise<AutomationRuleValidationResult> {
  return invokeTauri(TauriCommand.automationRepository.validateActivation, {
    rule: toAutomationRuleInput(rule),
    globalConfig,
    tags: Array.isArray(tags) ? tags : tags ? [tags] : [],
  });
}

function toAutomationRuleInput(rule: AutomationRule) {
  return {
    ...rule,
    saveHistory: rule.saveHistory ?? rule.projectId !== 'none',
    tagIds: rule.tagIds ?? (
      rule.projectId && rule.projectId !== 'none' && rule.projectId !== 'inbox'
        ? [rule.projectId]
        : []
    ),
  };
}
