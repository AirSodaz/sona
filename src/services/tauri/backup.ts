import type {
  BackupManifestV1,
  BackupWebDavConfig,
  BackupWebDavTestResult,
  RemoteBackupEntry,
} from '../../types/backup';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

type ExportBackupArchiveRequest =
  TauriCommandArgs<typeof TauriCommand.backup.exportArchive>['request'];

export async function exportBackupArchive<TResult = BackupManifestV1>(
  request: ExportBackupArchiveRequest,
): Promise<TResult> {
  return invokeTauri(TauriCommand.backup.exportArchive, { request }) as Promise<TResult>;
}

export async function prepareBackupImport<TResult = unknown>(
  archivePath: string,
): Promise<TResult> {
  return invokeTauri(TauriCommand.backup.prepareImport, { archivePath }) as Promise<TResult>;
}

export async function applyPreparedHistoryImport(importId: string): Promise<void> {
  await invokeTauri(TauriCommand.backup.applyPreparedImport, { importId });
}

export async function disposePreparedBackupImport(importId: string): Promise<void> {
  await invokeTauri(TauriCommand.backup.disposePreparedImport, { importId });
}

export async function webdavTestConnection(
  config: BackupWebDavConfig,
): Promise<BackupWebDavTestResult> {
  return invokeTauri(TauriCommand.backup.webdavTestConnection, { config });
}

export async function webdavListBackups(
  config: BackupWebDavConfig,
): Promise<RemoteBackupEntry[]> {
  return invokeTauri(TauriCommand.backup.webdavListBackups, { config });
}

export async function webdavUploadBackup(
  config: BackupWebDavConfig,
  localArchivePath: string,
): Promise<void> {
  await invokeTauri(TauriCommand.backup.webdavUploadBackup, {
    config,
    localArchivePath,
  });
}

export async function webdavDownloadBackup(
  config: BackupWebDavConfig,
  href: string,
  outputPath: string,
): Promise<void> {
  await invokeTauri(TauriCommand.backup.webdavDownloadBackup, {
    config,
    href,
    outputPath,
  });
}
