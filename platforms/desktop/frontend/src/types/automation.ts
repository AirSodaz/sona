import type { ExportFormat, ExportMode } from '../utils/exportFormats';

export type { AutomationRuleValidationResult } from '../bindings';

export interface AutomationStageConfig {
  autoPolish: boolean;
  polishPresetId?: string;
  autoTranslate: boolean;
  translationLanguage?: string;
}

export interface AutomationExportConfig {
  directory: string;
  format: ExportFormat;
  mode: ExportMode;
  prefix: string;
}

export interface AutomationRule {
  id: string;
  name: string;
  saveHistory: boolean;
  projectId: string;
  watchDirectory: string;
  recursive: boolean;
  enabled: boolean;
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
  attempt: number;
  filePath: string;
  sourceFingerprint: string;
  size: number;
  mtimeMs: number;
  status: 'pending' | 'complete' | 'error' | 'discarded';
  processedAt: number;
  historyId?: string | null;
  exportPath?: string | null;
  errorMessage?: string | null;
}
