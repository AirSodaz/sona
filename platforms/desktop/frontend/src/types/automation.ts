import type { ExportFormat, ExportMode } from '../utils/exportFormats';
export type { AutomationRuleValidationResult } from '../bindings';

export type AutomationPresetId = 'meeting_notes' | 'lecture_notes' | 'bilingual_subtitles' | 'custom';

export type BuiltInAutomationPresetId = Exclude<AutomationPresetId, 'custom'>;

export type AutomationRuleKind = 'tag' | 'file';

export interface AutomationProfile {
  id: string;
  name: string;
  translationLanguage: string;
  polishPresetId: string;
  summaryTemplateId: string;
  enabledTextReplacementSetIds: string[];
  enabledHotwordSetIds: string[];
  enabledPolishKeywordSetIds: string[];
  enabledSpeakerProfileIds: string[];
  createdAt: number;
  updatedAt: number;
}

export interface AutomationActions {
  autoPolish: boolean;
  autoTranslate: boolean;
  autoSummary: boolean;
}

export interface AutomationStageConfig {
  autoPolish: boolean;
  polishPresetId?: string;
  autoTranslate: boolean;
  translationLanguage?: string;
  autoSummary?: boolean;
  exportEnabled: boolean;
}

export interface AutomationResolutionSnapshot {
  fileRuleId?: string;
  tagRuleId?: string;
  profileId?: string;
  profileSource: 'file' | 'tag' | 'global';
  actions: AutomationActions;
  resolvedAt: number;
}

export interface AutomationExportConfig {
  directory: string;
  format: ExportFormat;
  mode: ExportMode;
  prefix?: string;
}

export interface AutomationRule {
  id: string;
  name: string;
  kind?: AutomationRuleKind;
  priority?: number;
  profileId?: string;
  profileSource?: 'explicit' | 'tag_match' | string;
  saveHistory?: boolean;
  tagIds?: string[];
  /** @deprecated Single-target compatibility alias. */
  projectId?: string;
  presetId: AutomationPresetId;
  watchDirectory: string;
  recursive: boolean;
  enabled: boolean;
  actions?: AutomationActions;
  migrationNotice?: string;
  stageConfig: AutomationStageConfig;
  exportConfig: AutomationExportConfig;
  createdAt: number;
  updatedAt: number;
}

export type AutomationRuntimeStatus = 'stopped' | 'watching' | 'scanning' | 'error';
export type AutomationRuntimeBlockReason =
  | 'already_processed'
  | 'already_pending'
  | 'recovery_blocked'
  | 'project_missing'
  | 'retry_source_missing';

export interface AutomationRuntimeState {
  ruleId: string;
  status: AutomationRuntimeStatus;
  lastScanAt?: number;
  lastCandidateAt?: number;
  lastQueuedAt?: number;
  lastBlockedAt?: number;
  lastBlockedReason?: AutomationRuntimeBlockReason;
  lastBlockedFilePath?: string;
  lastProcessedAt?: number;
  lastResult?: 'success' | 'error';
  lastResultMessage?: string;
  lastProcessedFilePath?: string;
  failureCount: number;
}

export interface AutomationProcessedEntry {
  id?: string;
  ruleId: string;
  kind?: AutomationRuleKind;
  inputVersion?: string;
  attempt?: number;
  filePath: string;
  sourceFingerprint: string;
  size: number;
  mtimeMs: number;
  status: 'pending' | 'complete' | 'error' | 'discarded';
  processedAt: number;
  historyId?: string;
  exportPath?: string;
  errorMessage?: string;
}

export interface AutomationPresetDefinition {
  id: AutomationPresetId;
  labelKey: string;
  defaultLabel: string;
  descriptionKey: string;
  defaultDescription: string;
  stageConfig: AutomationStageConfig;
  exportConfig: Pick<AutomationExportConfig, 'format' | 'mode'>;
}
