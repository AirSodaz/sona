export type SyncPresetV1 = 'content' | 'standard' | 'full';
export type SyncLifecycleState =
  | 'disabled'
  | 'locked'
  | 'idle'
  | 'syncing'
  | 'paused'
  | 'error';

export interface SyncErrorSnapshot {
  code: string;
  message: string;
  retryable: boolean;
}

export interface SyncStatusSnapshot {
  state: SyncLifecycleState;
  providerId: string | null;
  vaultId: string | null;
  preset: SyncPresetV1 | null;
  lastSuccessAtMs: number | null;
  pendingOperationCount: number;
  conflictCount: number;
  nextRetryAtMs: number | null;
  lastError: SyncErrorSnapshot | null;
}

export interface SyncRunResult {
  pulledSegmentCount: number;
  pulledCheckpointCount: number;
  pushedSegmentCount: number;
  appliedOperationCount: number;
  publishedOperationCount: number;
  conflictCount: number;
  checkpointPublished: boolean;
}

export interface SyncJoinPreview {
  localOperationCount: number;
  remoteOperationCount: number;
  projectedConflictCount: number;
}

export interface SyncProviderDescriptor {
  id: string;
  displayName: string;
}

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

export type SyncConflictResolution =
  | 'keep_current'
  | 'use_conflicting'
  | 'keep_both';
export type SyncConflictKind = 'concurrent_write' | 'delete_vs_write';
export type SyncEntityKind =
  | 'project'
  | 'history_item'
  | 'history_transcript'
  | 'history_summary'
  | 'transcript_snapshot'
  | 'setting'
  | 'summary_template'
  | 'polish_preset'
  | 'vocabulary_set'
  | 'vocabulary_rule'
  | 'speaker_profile'
  | 'automation_rule'
  | 'credential_profile';

export interface SyncEntityKey {
  kind: SyncEntityKind;
  id: string;
}

export interface SyncOperation {
  operationId: string;
  sourceDeviceId: string;
  sourceSequence: number;
  causalContext: {
    observedSequences: Record<string, number>;
  };
  version: {
    clock: {
      physicalMs: number;
      logical: number;
    };
    deviceId: string;
    operationId: string;
  };
  entity: SyncEntityKey;
  kind:
    | { kind: 'set_field'; field: string; value: unknown }
    | { kind: 'delete_entity' };
}

export interface SyncConflictSummary {
  conflictId: string;
  kind: SyncConflictKind;
  entity: SyncEntityKey;
  field: string | null;
  createdAtMs: number;
}

export interface SyncConflictDetail {
  summary: SyncConflictSummary;
  current: SyncOperation;
  conflicting: SyncOperation;
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
