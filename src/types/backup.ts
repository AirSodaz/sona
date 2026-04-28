import type { AutomationProcessedEntry, AutomationRule } from './automation';
import type { AppConfig } from './config';
import type { HistoryItem } from './history';
import type { ProjectRecord } from './project';
import type { HistorySummaryPayload, TranscriptSegment } from './transcript';

export const BACKUP_SCHEMA_VERSION = 1 as const;
export const BACKUP_HISTORY_MODE = 'light' as const;

export interface BackupManifestV1 {
  schemaVersion: typeof BACKUP_SCHEMA_VERSION;
  createdAt: string;
  appVersion: string;
  historyMode: typeof BACKUP_HISTORY_MODE;
  scopes: {
    config: true;
    workspace: true;
    history: true;
    automation: true;
    analytics: true;
  };
  counts: {
    projects: number;
    historyItems: number;
    transcriptFiles: number;
    summaryFiles: number;
    automationRules: number;
    automationProcessedEntries: number;
    analyticsFiles: number;
  };
}

export interface ExportBackupResult {
  archivePath: string;
  manifest: BackupManifestV1;
}

export interface PreparedBackupImport {
  archivePath: string;
  extractionDir: string;
  manifest: BackupManifestV1;
  config: AppConfig;
  projects: ProjectRecord[];
  historyItems: HistoryItem[];
  transcriptFiles: Record<string, TranscriptSegment[]>;
  summaryFiles: Record<string, HistorySummaryPayload>;
  automationRules: AutomationRule[];
  automationProcessedEntries: AutomationProcessedEntry[];
  analyticsContent: string;
}

export type BackupOperationBlocker = 'recording' | 'batch_queue';
