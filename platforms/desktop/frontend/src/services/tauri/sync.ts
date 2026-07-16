import type {
  SyncChangePasswordRequest,
  LegacyRemoteBackupListResult,
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncJoinRequest,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
} from '../../types/sync';
import type { PreparedBackupImport } from '../../types/backup';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export const getSyncStatus = (): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.getStatus);

export const testWebDavSyncProvider = (
  config: WebDavObjectStoreConfig,
): Promise<SyncProviderDescriptor> =>
  invokeTauri(TauriCommand.sync.testWebDavProvider, { config });

export const listLegacyRemoteBackups = (
  config: WebDavObjectStoreConfig,
): Promise<LegacyRemoteBackupListResult> =>
  invokeTauri(TauriCommand.sync.listLegacyBackups, { config });

export const prepareLegacyRemoteBackupImport = (
  config: WebDavObjectStoreConfig,
  key: string,
): Promise<PreparedBackupImport> =>
  invokeTauri(TauriCommand.sync.prepareLegacyBackupImport, { config, key });

export const createSyncVault = (
  request: SyncCreateRequest,
): Promise<SyncCreateResult> =>
  invokeTauri(TauriCommand.sync.createVault, { request });

export const previewSyncJoin = (
  request: SyncPreviewJoinRequest,
): Promise<SyncJoinPreview> =>
  invokeTauri(TauriCommand.sync.previewJoin, { request });

export const joinSyncVault = (request: SyncJoinRequest): Promise<SyncRunResult> =>
  invokeTauri(TauriCommand.sync.joinVault, { request });

export const unlockSyncVault = (
  request: SyncUnlockRequest,
): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.unlock, { request });

export const unlockSyncVaultWithRecovery = (
  request: SyncUnlockRecoveryRequest,
): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.unlockWithRecovery, { request });

export const lockSyncVault = (): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.lock);

export const setSyncPaused = (paused: boolean): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.setPaused, { paused });

export const disconnectSyncVault = (): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.disconnect);

export const runSyncNow = (): Promise<SyncRunResult> =>
  invokeTauri(TauriCommand.sync.runNow);

export const changeSyncPreset = (
  preset: SyncPresetV1,
  confirmShrink: boolean,
): Promise<SyncStatusSnapshot> =>
  invokeTauri(TauriCommand.sync.changePreset, { preset, confirmShrink });

export const changeSyncMasterPassword = (
  request: SyncChangePasswordRequest,
): Promise<void> =>
  invokeTauri(TauriCommand.sync.changeMasterPassword, { request });

export const generateSyncRecoveryKey = (): Promise<string> =>
  invokeTauri(TauriCommand.sync.generateRecoveryKey);

export const listSyncConflicts = (): Promise<SyncConflictSummary[]> =>
  invokeTauri(TauriCommand.sync.listConflicts);

export const getSyncConflict = (
  conflictId: string,
): Promise<SyncConflictDetail | null> =>
  invokeTauri(TauriCommand.sync.getConflict, { conflictId });

export const resolveSyncConflict = (
  conflictId: string,
  resolution: SyncConflictResolution,
): Promise<void> =>
  invokeTauri(TauriCommand.sync.resolveConflict, { conflictId, resolution });
