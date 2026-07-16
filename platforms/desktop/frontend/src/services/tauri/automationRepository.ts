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
import type { ProjectRecord } from '../../types/project';
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
