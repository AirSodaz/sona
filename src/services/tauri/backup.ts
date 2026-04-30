import type {
  BackupManifestV1,
  BackupWebDavConfig,
  BackupWebDavTestResult,
  RemoteBackupEntry,
} from '../../types/backup';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function exportBackupArchive<TResult = BackupManifestV1>(request: {
  archivePath: string;
  appVersion: string;
  config: unknown;
  projects: unknown[];
  automationRules: unknown[];
  automationProcessedEntries: unknown[];
  analyticsContent: string;
}): Promise<TResult> {
  return invokeTauri<TResult>(TauriCommand.backup.exportArchive, { request });
}

export async function prepareBackupImport<TResult = unknown>(
  archivePath: string,
): Promise<TResult> {
  return invokeTauri<TResult>(TauriCommand.backup.prepareImport, { archivePath });
}

export async function applyPreparedHistoryImport(importId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.backup.applyPreparedImport, { importId });
}

export async function disposePreparedBackupImport(importId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.backup.disposePreparedImport, { importId });
}

export async function webdavTestConnection(
  config: BackupWebDavConfig,
): Promise<BackupWebDavTestResult> {
  return invokeTauri<BackupWebDavTestResult>(TauriCommand.backup.webdavTestConnection, { config });
}

export async function webdavListBackups(
  config: BackupWebDavConfig,
): Promise<RemoteBackupEntry[]> {
  return invokeTauri<RemoteBackupEntry[]>(TauriCommand.backup.webdavListBackups, { config });
}

export async function webdavUploadBackup(
  config: BackupWebDavConfig,
  localArchivePath: string,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.backup.webdavUploadBackup, {
    config,
    localArchivePath,
  });
}

export async function webdavDownloadBackup(
  config: BackupWebDavConfig,
  href: string,
  outputPath: string,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.backup.webdavDownloadBackup, {
    config,
    href,
    outputPath,
  });
}
