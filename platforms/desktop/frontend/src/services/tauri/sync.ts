import type { PreparedBackupImport } from '../../types/backup';
import type {
  AnySyncProviderConfig,
  DiscoveredVaultSummary,
  LegacyRemoteBackupListResult,
  S3ObjectStoreConfig,
  SyncChangePasswordRequest,
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncCreateRequest,
  SyncCreateResult,
  SyncCreateTransportRequest,
  SyncJoinPreview,
  SyncJoinRequest,
  SyncPairingInfo,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncPreviewJoinTransportRequest,
  SyncProviderDescriptor,
  SyncProviderTransportInput,
  SyncRunResult,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
} from '../../types/sync';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export const getSyncStatus = (): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.getStatus);

export const webDavProviderInput = (
  configuration: WebDavObjectStoreConfig
): SyncProviderTransportInput => ({
  providerId: 'webdav',
  configuration,
});

export const s3ProviderInput = (
  configuration: S3ObjectStoreConfig
): SyncProviderTransportInput => ({
  providerId: 's3',
  configuration,
});

export const toSyncProviderTransportInput = (
  provider: SyncProviderTransportInput | WebDavObjectStoreConfig | S3ObjectStoreConfig
): SyncProviderTransportInput => {
  if ('providerId' in provider && typeof provider.providerId === 'string') {
    return provider as SyncProviderTransportInput;
  }
  if ('bucket' in provider) {
    return s3ProviderInput(provider as S3ObjectStoreConfig);
  }
  return webDavProviderInput(provider as WebDavObjectStoreConfig);
};

const createTransportRequest = (request: SyncCreateRequest): SyncCreateTransportRequest => ({
  ...request,
  provider: toSyncProviderTransportInput(request.provider),
});

const joinTransportRequest = (
  request: SyncPreviewJoinRequest
): SyncPreviewJoinTransportRequest => ({
  ...request,
  provider: toSyncProviderTransportInput(request.provider),
});
export const testWebDavSyncProvider = (
  config: WebDavObjectStoreConfig
): Promise<SyncProviderDescriptor> =>
  invokeTauri(TauriCommand.sync.testProvider, {
    provider: webDavProviderInput(config),
  });

export const discoverWebDavSyncVaults = (
  config: WebDavObjectStoreConfig
): Promise<DiscoveredVaultSummary[]> =>
  invokeTauri(TauriCommand.sync.discoverWebDavVaults, { config });

export const testS3SyncProvider = (config: S3ObjectStoreConfig): Promise<SyncProviderDescriptor> =>
  invokeTauri(TauriCommand.sync.testProvider, { provider: s3ProviderInput(config) });

export const discoverS3SyncVaults = (
  config: S3ObjectStoreConfig
): Promise<DiscoveredVaultSummary[]> =>
  invokeTauri(TauriCommand.sync.discoverVaults, { provider: s3ProviderInput(config) });
export const testSyncProvider = (
  config: AnySyncProviderConfig
): Promise<SyncProviderDescriptor> => {
  if ('bucket' in config) {
    return testS3SyncProvider(config);
  }
  return testWebDavSyncProvider(config);
};

export const discoverSyncVaults = (
  config: AnySyncProviderConfig
): Promise<DiscoveredVaultSummary[]> => {
  if ('bucket' in config) {
    return discoverS3SyncVaults(config);
  }
  return discoverWebDavSyncVaults(config);
};

export const getSyncPairingInfo = (): Promise<SyncPairingInfo | null> =>
  invokeTauri(TauriCommand.sync.getPairingInfo);

export const listLegacyRemoteBackups = (
  config: WebDavObjectStoreConfig
): Promise<LegacyRemoteBackupListResult> =>
  invokeTauri(TauriCommand.sync.listLegacyBackups, { config });

export const prepareLegacyRemoteBackupImport = (
  config: WebDavObjectStoreConfig,
  key: string
): Promise<PreparedBackupImport> =>
  invokeTauri(TauriCommand.sync.prepareLegacyBackupImport, { config, key });

export const createSyncVault = (request: SyncCreateRequest): Promise<SyncCreateResult> =>
  invokeTauri(TauriCommand.sync.createVault, {
    request: createTransportRequest(request),
  });

export const previewSyncJoin = (request: SyncPreviewJoinRequest): Promise<SyncJoinPreview> =>
  invokeTauri(TauriCommand.sync.previewJoin, {
    request: joinTransportRequest(request),
  });

export const joinSyncVault = (request: SyncJoinRequest): Promise<SyncRunResult> =>
  invokeTauri(TauriCommand.sync.joinVault, {
    request: joinTransportRequest(request),
  });

export const unlockSyncVault = (request: SyncUnlockRequest): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.unlock, { request });

export const unlockSyncVaultWithRecovery = (
  request: SyncUnlockRecoveryRequest
): Promise<SyncStatusSnapshot> => invokeTauri(TauriCommand.sync.unlockWithRecovery, { request });

export const lockSyncVault = (): Promise<SyncStatusSnapshot> => invokeTauri(TauriCommand.sync.lock);

export const setSyncPaused = (paused: boolean): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.setPaused, { paused });

export const disconnectSyncVault = (): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.disconnect);

export const runSyncNow = (): Promise<SyncRunResult> => invokeTauri(TauriCommand.sync.runNow);

export const changeSyncPreset = (
  preset: SyncPresetV1,
  confirmShrink: boolean
): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.changePreset, { preset, confirmShrink });

export const changeSyncMasterPassword = (request: SyncChangePasswordRequest): Promise<void> =>
  invokeTauri(TauriCommand.sync.changeMasterPassword, { request });

export const generateSyncRecoveryKey = (): Promise<string> =>
  invokeTauri(TauriCommand.sync.generateRecoveryKey);

export const listSyncConflicts = (): Promise<SyncConflictSummary[]> =>
  invokeTauri(TauriCommand.sync.listConflicts);

export const getSyncConflict = (conflictId: string): Promise<SyncConflictDetail | null> =>
  invokeTauri(TauriCommand.sync.getConflict, { conflictId });

export const resolveSyncConflict = (
  conflictId: string,
  resolution: SyncConflictResolution
): Promise<void> => invokeTauri(TauriCommand.sync.resolveConflict, { conflictId, resolution });
