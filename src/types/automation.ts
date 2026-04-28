import type { ExportFormat, ExportMode } from '../utils/exportFormats';

export type AutomationPresetId = 'meeting_notes' | 'lecture_notes' | 'bilingual_subtitles' | 'custom';

export type BuiltInAutomationPresetId = Exclude<AutomationPresetId, 'custom'>;

export interface AutomationStageConfig {
  autoPolish: boolean;
  polishPresetId?: string;
  autoTranslate: boolean;
  translationLanguage?: string;
  exportEnabled: boolean;
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
  projectId: string;
  presetId: AutomationPresetId;
  watchDirectory: string;
  recursive: boolean;
  enabled: boolean;
  stageConfig: AutomationStageConfig;
  exportConfig: AutomationExportConfig;
  createdAt: number;
  updatedAt: number;
}

export type AutomationRuntimeStatus = 'stopped' | 'watching' | 'scanning' | 'error';

export interface AutomationRuntimeState {
  ruleId: string;
  status: AutomationRuntimeStatus;
  lastScanAt?: number;
  lastProcessedAt?: number;
  lastResult?: 'success' | 'error';
  lastResultMessage?: string;
  lastProcessedFilePath?: string;
  failureCount: number;
}

export interface AutomationProcessedEntry {
  ruleId: string;
  filePath: string;
  sourceFingerprint: string;
  size: number;
  mtimeMs: number;
  status: 'complete' | 'error' | 'discarded';
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

export interface AutomationRuleValidationResult {
  valid: boolean;
  code?: string;
  message?: string;
}
