import type { AppConfig } from '../../types/config';
import type {
  AutomationActions,
  AutomationProcessedEntry,
  AutomationProfile,
  AutomationRule,
  AutomationRuleValidationResult,
} from '../../types/automation';
import type { TagRecord } from '../../types/tag';
import {
  AutomationProcessedRecord_Serialize,
  AutomationProfileRecord,
  AutomationRuleRecord,
} from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface AutomationRepositoryState {
  profiles: AutomationProfile[];
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
}

const EXPORT_FORMATS = new Set(['srt', 'json', 'txt', 'vtt', 'md']);
const EXPORT_MODES = new Set(['original', 'translation', 'bilingual']);
const PROCESSED_STATUSES = new Set(['pending', 'complete', 'error', 'discarded']);
const AUTOMATION_PRESET_IDS = new Set(['meeting_notes', 'lecture_notes', 'bilingual_subtitles', 'custom']);

function normalizeActions(actions?: Partial<AutomationActions>): AutomationActions {
  return {
    autoPolish: actions?.autoPolish === true,
    autoTranslate: actions?.autoTranslate === true,
    autoSummary: actions?.autoSummary === true,
  };
}

function normalizeProfile(record: AutomationProfileRecord): AutomationProfile {
  return { ...record };
}

function normalizeAutomationRule(record: AutomationRuleRecord): AutomationRule {
  return {
    ...record,
    migrationNotice: record.migrationNotice ?? undefined,
    kind: record.kind === 'tag' ? 'tag' : 'file',
    priority: Number(record.priority || 0),
    profileId: record.profileId ?? undefined,
    profileSource: record.profileSource || 'tag_match',
    actions: normalizeActions(record.actions),
    tagIds: record.tagIds || [],
    presetId: AUTOMATION_PRESET_IDS.has(record.presetId)
      ? record.presetId as AutomationRule['presetId'] : 'custom',
    stageConfig: {
      ...record.stageConfig,
      polishPresetId: record.stageConfig.polishPresetId || undefined,
      translationLanguage: record.stageConfig.translationLanguage || undefined,
    },
    exportConfig: {
      ...record.exportConfig,
      format: EXPORT_FORMATS.has(record.exportConfig.format)
        ? record.exportConfig.format as AutomationRule['exportConfig']['format'] : 'txt',
      mode: EXPORT_MODES.has(record.exportConfig.mode)
        ? record.exportConfig.mode as AutomationRule['exportConfig']['mode'] : 'original',
      prefix: record.exportConfig.prefix || undefined,
    },
  };
}

function normalizeAutomationProcessedEntry(record: AutomationProcessedRecord_Serialize): AutomationProcessedEntry {
  const extendedRecord = record as AutomationProcessedRecord_Serialize & {
    kind?: string;
    inputVersion?: string;
    attempt?: number;
  };
  return {
    id: record.id,
    ruleId: record.ruleId,
    kind: extendedRecord.kind === 'tag' ? 'tag' : 'file',
    inputVersion: extendedRecord.inputVersion || record.sourceFingerprint,
    attempt: Number(extendedRecord.attempt || 1),
    filePath: record.filePath,
    sourceFingerprint: record.sourceFingerprint,
    size: record.size,
    mtimeMs: record.mtimeMs,
    status: PROCESSED_STATUSES.has(record.status) ? record.status as AutomationProcessedEntry['status'] : 'error',
    processedAt: record.processedAt,
    historyId: record.historyId ?? undefined,
    exportPath: record.exportPath ?? undefined,
    errorMessage: record.errorMessage ?? undefined,
  };
}

export async function automationLoadRepositoryState(): Promise<AutomationRepositoryState> {
  const state = await invokeTauri(TauriCommand.automationRepository.loadState);
  return {
    profiles: state.profiles.map(normalizeProfile),
    rules: state.rules.map(normalizeAutomationRule),
    processedEntries: state.processedEntries.map(normalizeAutomationProcessedEntry),
  };
}

function toAutomationRuleInput(rule: AutomationRule) {
  return {
    ...rule,
    kind: rule.kind ?? 'file',
    priority: rule.priority ?? 0,
    profileSource: rule.profileSource ?? 'tag_match',
    actions: normalizeActions(rule.actions),
    saveHistory: rule.saveHistory ?? true,
    tagIds: rule.tagIds ?? [],
  };
}

function toAutomationProfileInput(profile: AutomationProfile) {
  return { ...profile };
}

export async function automationPersistProfiles(profiles: AutomationProfile[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistProfiles, {
    profiles: profiles.map(toAutomationProfileInput),
  });
}

export async function automationPersistRules(rules: AutomationRule[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistRules, {
    rules: rules.map(toAutomationRuleInput),
  });
}

export async function automationPersistProcessedEntries(processedEntries: AutomationProcessedEntry[]): Promise<void> {
  await invokeTauri(TauriCommand.automationRepository.persistProcessedEntries, { processedEntries });
}

export async function automationPersistRepositoryState(
  profilesOrRules: AutomationProfile[] | AutomationRule[],
  rulesOrProcessed: AutomationRule[] | AutomationProcessedEntry[],
  maybeProcessed?: AutomationProcessedEntry[],
): Promise<void> {
  const profiles = maybeProcessed ? profilesOrRules as AutomationProfile[] : [];
  const rules = maybeProcessed ? rulesOrProcessed as AutomationRule[] : profilesOrRules as AutomationRule[];
  const processedEntries = maybeProcessed ?? rulesOrProcessed as AutomationProcessedEntry[];
  await invokeTauri(TauriCommand.automationRepository.persistState, {
    profiles: profiles.map(toAutomationProfileInput),
    rules: rules.map(toAutomationRuleInput),
    processedEntries,
  });
}

export async function automationValidateRuleActivation(
  rule: AutomationRule, globalConfig: AppConfig, tags: TagRecord[] | TagRecord | null,
): Promise<AutomationRuleValidationResult> {
  return invokeTauri(TauriCommand.automationRepository.validateActivation, {
    rule: toAutomationRuleInput(rule), globalConfig,
    tags: Array.isArray(tags) ? tags : tags ? [tags] : [],
  });
}
