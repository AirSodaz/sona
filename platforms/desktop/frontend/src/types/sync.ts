import type {
  SyncPresetV1,
  SyncStatusSnapshot,
} from '../bindings';

export type {
  SyncConflictDetail,
  SyncConflictKind,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncEntityKey,
  SyncEntityKind,
  SyncErrorSnapshot,
  SyncJoinPreview,
  SyncLifecycleState,
  SyncOperation,
  SyncPresetV1,
  SyncProviderDescriptor,
  SyncRunResult,
  SyncStatusSnapshot,
} from '../bindings';

export interface WebDavObjectStoreConfig {
  serverUrl: string;
  remoteRoot: string;
  username: string;
  password: string;
}

export interface LegacyRemoteBackupEntry {
  key: string;
  fileName: string;
  size: number;
  modifiedAt: string | null;
}

export interface LegacyRemoteBackupListResult {
  entries: LegacyRemoteBackupEntry[];
  credentialsMigrated: boolean;
}

export interface SyncCreateRequest {
  provider: WebDavObjectStoreConfig;
  preset: SyncPresetV1;
  masterPassword: string;
  createRecoveryKey: boolean;
}

export interface SyncCreateResult {
  vaultId: string;
  deviceId: string;
  recoveryKey: string | null;
  status: SyncStatusSnapshot;
}

export interface SyncPreviewJoinRequest {
  provider: WebDavObjectStoreConfig;
  vaultId: string;
  masterPassword: string;
}

export type SyncJoinRequest = SyncPreviewJoinRequest;

export interface SyncUnlockRequest {
  providerPassword: string;
  masterPassword: string;
}

export interface SyncUnlockRecoveryRequest {
  providerPassword: string;
  recoveryKey: string;
}

export interface SyncChangePasswordRequest {
  currentMasterPassword: string;
  nextMasterPassword: string;
}

export const DISABLED_SYNC_STATUS: SyncStatusSnapshot = {
  state: 'disabled',
  providerId: null,
  vaultId: null,
  preset: null,
  lastSuccessAtMs: null,
  pendingOperationCount: 0,
  conflictCount: 0,
  nextRetryAtMs: null,
  lastError: null,
};
