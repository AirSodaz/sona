import type {
  BackupManifestV1,
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
