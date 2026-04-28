import { invoke } from '@tauri-apps/api/core';
import { join, tempDir } from '@tauri-apps/api/path';
import { mkdir, remove } from '@tauri-apps/plugin-fs';
import {
  buildDefaultBackupFileName,
  exportBackup,
  getBackupOperationBlocker,
  prepareImportBackup,
} from './backupService';
import { settingsStore, STORE_KEY_BACKUP_WEBDAV } from './storageService';
import type {
  BackupWebDavConfig,
  BackupWebDavTestResult,
  PreparedBackupImport,
  RemoteBackupEntry,
  UploadRemoteBackupResult,
} from '../types/backup';

function normalizeWebDavConfig(config: Partial<BackupWebDavConfig> | null | undefined): BackupWebDavConfig {
  return {
    serverUrl: typeof config?.serverUrl === 'string' ? config.serverUrl.trim() : '',
    remoteDir: typeof config?.remoteDir === 'string' ? config.remoteDir.trim() : '',
    username: typeof config?.username === 'string' ? config.username.trim() : '',
    password: typeof config?.password === 'string' ? config.password : '',
  };
}

function validateWebDavConfig(config: BackupWebDavConfig): BackupWebDavConfig {
  const normalized = normalizeWebDavConfig(config);
  if (!normalized.serverUrl) {
    throw new Error('WebDAV server URL is required.');
  }

  let parsedUrl: URL;
  try {
    parsedUrl = new URL(normalized.serverUrl);
  } catch {
    throw new Error('WebDAV server URL is invalid.');
  }

  if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
    throw new Error('WebDAV server URL must start with http:// or https://.');
  }

  if (!normalized.username) {
    throw new Error('WebDAV username is required.');
  }

  if (!normalized.password.trim()) {
    throw new Error('WebDAV password is required.');
  }

  return normalized;
}

function normalizeRemoteEntries(entries: RemoteBackupEntry[]): RemoteBackupEntry[] {
  return [...entries]
    .filter((entry) => entry.fileName.endsWith('.tar.bz2'))
    .sort((left, right) => {
      const leftTime = left.modifiedAt ? new Date(left.modifiedAt).getTime() : 0;
      const rightTime = right.modifiedAt ? new Date(right.modifiedAt).getTime() : 0;
      const safeLeftTime = Number.isNaN(leftTime) ? 0 : leftTime;
      const safeRightTime = Number.isNaN(rightTime) ? 0 : rightTime;
      return safeRightTime - safeLeftTime;
    });
}

function ensureCloudTransferOperationsIdle(): void {
  const blocker = getBackupOperationBlocker();
  if (!blocker) {
    return;
  }

  if (blocker === 'recording') {
    throw new Error('Stop Live Record before exporting or importing backups.');
  }

  throw new Error('Wait for Batch Import to finish or clear pending items before exporting or importing backups.');
}

async function createTemporaryDirectory(prefix: string): Promise<string> {
  const systemTempDir = await tempDir();
  const dir = await join(
    systemTempDir,
    `sona-${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
  );
  await mkdir(dir, { recursive: true });
  return dir;
}

async function cleanupAbsolutePath(path: string): Promise<void> {
  try {
    await remove(path, { recursive: true });
  } catch {
    // best-effort cleanup
  }
}

async function resolveWebDavConfig(input?: BackupWebDavConfig): Promise<BackupWebDavConfig> {
  if (input) {
    return validateWebDavConfig(input);
  }

  return validateWebDavConfig(await loadConfig());
}

export async function loadConfig(): Promise<BackupWebDavConfig> {
  const saved = await settingsStore.get<BackupWebDavConfig | null>(STORE_KEY_BACKUP_WEBDAV);
  return normalizeWebDavConfig(saved);
}

export async function saveConfig(config: BackupWebDavConfig): Promise<void> {
  const normalized = normalizeWebDavConfig(config);
  await settingsStore.set(STORE_KEY_BACKUP_WEBDAV, normalized);
  await settingsStore.save();
}

export async function testConnection(config?: BackupWebDavConfig): Promise<BackupWebDavTestResult> {
  const resolvedConfig = await resolveWebDavConfig(config);
  return invoke<BackupWebDavTestResult>('webdav_test_connection', {
    config: resolvedConfig,
  });
}

export async function listBackups(config?: BackupWebDavConfig): Promise<RemoteBackupEntry[]> {
  ensureCloudTransferOperationsIdle();
  const resolvedConfig = await resolveWebDavConfig(config);
  const entries = await invoke<RemoteBackupEntry[]>('webdav_list_backups', {
    config: resolvedConfig,
  });
  return normalizeRemoteEntries(entries);
}

export async function uploadBackup(config?: BackupWebDavConfig): Promise<UploadRemoteBackupResult> {
  ensureCloudTransferOperationsIdle();
  const resolvedConfig = await resolveWebDavConfig(config);
  const tempExportDir = await createTemporaryDirectory('webdav-backup-upload');
  const archivePath = await join(tempExportDir, buildDefaultBackupFileName());

  try {
    const result = await exportBackup({ archivePath });
    if (!result) {
      throw new Error('Backup export was cancelled before the archive was created.');
    }

    await invoke('webdav_upload_backup', {
      config: resolvedConfig,
      localArchivePath: archivePath,
    });

    return {
      fileName: result.archivePath.split(/[/\\]/).pop() || buildDefaultBackupFileName(),
      manifest: result.manifest,
    };
  } finally {
    await cleanupAbsolutePath(tempExportDir);
  }
}

export async function prepareImportFromRemote(
  entry: RemoteBackupEntry,
  config?: BackupWebDavConfig,
): Promise<PreparedBackupImport> {
  ensureCloudTransferOperationsIdle();
  const resolvedConfig = await resolveWebDavConfig(config);
  const tempDownloadDir = await createTemporaryDirectory('webdav-backup-download');
  const archivePath = await join(tempDownloadDir, entry.fileName);

  try {
    await invoke('webdav_download_backup', {
      config: resolvedConfig,
      href: entry.href,
      outputPath: archivePath,
    });

    return await prepareImportBackup({ archivePath }) as PreparedBackupImport;
  } finally {
    await cleanupAbsolutePath(tempDownloadDir);
  }
}

export const backupWebDavService = {
  loadConfig,
  saveConfig,
  testConnection,
  listBackups,
  uploadBackup,
  prepareImportFromRemote,
};
