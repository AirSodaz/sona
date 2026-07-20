import type {
  BackupManifest_Serialize as GeneratedBackupManifest,
  PreparedBackupImport_Serialize as GeneratedPreparedBackupImport,
} from '../bindings';
import type { AutomationProcessedEntry, AutomationProfile, AutomationRule } from './automation';
import type { AppConfig } from './config';
import type { TagRecord } from './tag';

export const BACKUP_SCHEMA_VERSION = 3 as const;
export const BACKUP_HISTORY_MODE = 'light' as const;

export type BackupManifest = GeneratedBackupManifest;
// Compatibility alias for callers that still import the pre-v2 TypeScript name.
export type BackupManifestV1 = BackupManifest;

export interface ExportBackupResult {
  archivePath: string;
  manifest: BackupManifestV1;
}

export interface BackupWebDavConfig {
  serverUrl: string;
  remoteDir: string;
  username: string;
  password: string;
}

export interface RemoteBackupEntry {
  href: string;
  fileName: string;
  size: number;
  modifiedAt: string | null;
}

export type BackupWebDavConnectionStatus = 'success' | 'warning';

export interface BackupWebDavTestResult {
  status: BackupWebDavConnectionStatus;
  message: string;
}

export interface UploadRemoteBackupResult {
  fileName: string;
  manifest: BackupManifestV1;
}

export type PreparedBackupImport = Omit<
  GeneratedPreparedBackupImport,
  'config' | 'tags' | 'automationProfiles' | 'automationRules' | 'automationProcessedEntries'
> & {
  config: AppConfig;
  tags: TagRecord[];
  automationProfiles: AutomationProfile[];
  automationRules: AutomationRule[];
  automationProcessedEntries: AutomationProcessedEntry[];
};

export type BackupOperationBlocker = 'recording' | 'batch_queue';
