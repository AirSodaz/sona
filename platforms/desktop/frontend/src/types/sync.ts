import type { SyncPresetV1, SyncStatusSnapshot } from '../bindings';

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
export interface S3ObjectStoreConfig {
  endpoint: string;
  region: string;
  bucket: string;
  remoteRoot: string;
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken?: string;
  forcePathStyle?: boolean;
}

export type AnySyncProviderConfig = WebDavObjectStoreConfig | S3ObjectStoreConfig;

export interface SyncProviderTransportInput {
  providerId: string;
  configuration: unknown;
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

export interface DiscoveredVaultSummary {
  vaultId: string;
  preset: SyncPresetV1;
}

export interface SyncPairingInfo {
  providerId: string;
  vaultId: string;
  serverUrl?: string | null;
  remoteRoot?: string | null;
  username?: string | null;
  endpoint?: string | null;
  region?: string | null;
  bucket?: string | null;
  accessKeyId?: string | null;
  forcePathStyle?: boolean | null;
}

export interface SyncCreateRequest {
  provider: SyncProviderTransportInput | WebDavObjectStoreConfig | S3ObjectStoreConfig;
  vaultId?: string;
  preset: SyncPresetV1;
  masterPassword: string;
  createRecoveryKey: boolean;
}

export type SyncCreateTransportRequest = Omit<SyncCreateRequest, 'provider'> & {
  provider: SyncProviderTransportInput;
};

export interface SyncCreateResult {
  vaultId: string;
  deviceId: string;
  recoveryKey: string | null;
  status: SyncStatusSnapshot;
}

export interface SyncPreviewJoinRequest {
  provider: SyncProviderTransportInput | WebDavObjectStoreConfig | S3ObjectStoreConfig;
  vaultId: string;
  masterPassword: string;
}

export type SyncJoinRequest = SyncPreviewJoinRequest;

export type SyncPreviewJoinTransportRequest = Omit<SyncPreviewJoinRequest, 'provider'> & {
  provider: SyncProviderTransportInput;
};

export type SyncJoinTransportRequest = SyncPreviewJoinTransportRequest;

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
