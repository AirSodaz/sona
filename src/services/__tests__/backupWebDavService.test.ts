import { beforeEach, describe, expect, it, vi } from 'vitest';
import packageJson from '../../../package.json';

const testContext = vi.hoisted(() => ({
  exportBackupMock: vi.fn(),
  getBackupOperationBlockerMock: vi.fn(),
  invokeMock: vi.fn(),
  joinMock: vi.fn(async (...parts: string[]) => parts.join('/')),
  mkdirMock: vi.fn().mockResolvedValue(undefined),
  prepareImportBackupMock: vi.fn(),
  removeMock: vi.fn().mockResolvedValue(undefined),
  settingsStoreGetMock: vi.fn(),
  settingsStoreSaveMock: vi.fn().mockResolvedValue(undefined),
  settingsStoreSetMock: vi.fn().mockResolvedValue(undefined),
  tempDirMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: testContext.invokeMock,
}));

vi.mock('@tauri-apps/api/path', () => ({
  join: testContext.joinMock,
  tempDir: testContext.tempDirMock,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  mkdir: testContext.mkdirMock,
  remove: testContext.removeMock,
}));

vi.mock('../backupService', () => ({
  buildDefaultBackupFileName: vi.fn(() => 'sona-backup-test.tar.bz2'),
  exportBackup: testContext.exportBackupMock,
  getBackupOperationBlocker: testContext.getBackupOperationBlockerMock,
  prepareImportBackup: testContext.prepareImportBackupMock,
}));

vi.mock('../storageService', () => ({
  STORE_KEY_BACKUP_WEBDAV: 'sona-backup-webdav',
  settingsStore: {
    get: testContext.settingsStoreGetMock,
    save: testContext.settingsStoreSaveMock,
    set: testContext.settingsStoreSetMock,
  },
}));

import {
  listBackups,
  loadConfig,
  prepareImportFromRemote,
  saveConfig,
  testConnection,
  uploadBackup,
} from '../backupWebDavService';

describe('backupWebDavService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, 'now').mockReturnValue(123456);
    vi.spyOn(Math, 'random').mockReturnValue(0.123456789);
    testContext.getBackupOperationBlockerMock.mockReturnValue(null);
    testContext.tempDirMock.mockResolvedValue('/temp');
  });

  it('loads and saves WebDAV config under its dedicated store key', async () => {
    testContext.settingsStoreGetMock.mockResolvedValue({
      serverUrl: ' https://dav.example.com/root ',
      remoteDir: ' backups/sona ',
      username: ' demo ',
      password: 'secret',
    });

    const loaded = await loadConfig();
    await saveConfig({
      serverUrl: ' https://dav.example.com/root ',
      remoteDir: ' backups/sona ',
      username: ' demo ',
      password: 'secret',
    });

    expect(loaded).toEqual({
      serverUrl: 'https://dav.example.com/root',
      remoteDir: 'backups/sona',
      username: 'demo',
      password: 'secret',
    });
    expect(testContext.settingsStoreSetMock).toHaveBeenCalledWith('sona-backup-webdav', {
      serverUrl: 'https://dav.example.com/root',
      remoteDir: 'backups/sona',
      username: 'demo',
      password: 'secret',
    });
  });

  it('allows http endpoints for connection tests and returns warning results', async () => {
    testContext.invokeMock.mockResolvedValue({
      status: 'warning',
      message: 'HTTP transport is not encrypted.',
    });

    const result = await testConnection({
      serverUrl: 'http://nas.local/dav',
      remoteDir: 'sona',
      username: 'demo',
      password: 'secret',
    });

    expect(result).toEqual({
      status: 'warning',
      message: 'HTTP transport is not encrypted.',
    });
    expect(testContext.invokeMock).toHaveBeenCalledWith('webdav_test_connection', {
      config: {
        serverUrl: 'http://nas.local/dav',
        remoteDir: 'sona',
        username: 'demo',
        password: 'secret',
      },
    });
  });

  it('lists remote backups in descending modified-time order and ignores non-archive files', async () => {
    testContext.invokeMock.mockResolvedValue([
      {
        href: 'https://dav.example.com/backups/a.txt',
        fileName: 'a.txt',
        size: 1,
        modifiedAt: '2026-04-27T00:00:00.000Z',
      },
      {
        href: 'https://dav.example.com/backups/older.tar.bz2',
        fileName: 'older.tar.bz2',
        size: 10,
        modifiedAt: '2026-04-27T00:00:00.000Z',
      },
      {
        href: 'https://dav.example.com/backups/newer.tar.bz2',
        fileName: 'newer.tar.bz2',
        size: 20,
        modifiedAt: '2026-04-29T00:00:00.000Z',
      },
    ]);

    const result = await listBackups({
      serverUrl: 'https://dav.example.com',
      remoteDir: 'backups',
      username: 'demo',
      password: 'secret',
    });

    expect(result.map((entry) => entry.fileName)).toEqual([
      'newer.tar.bz2',
      'older.tar.bz2',
    ]);
  });

  it('uploads a local backup archive through WebDAV and cleans temporary files', async () => {
    testContext.exportBackupMock.mockResolvedValue({
      archivePath: '/temp/sona-webdav-backup-upload-123456-4fzzzxjy/sona-backup-test.tar.bz2',
      manifest: {
        schemaVersion: 1,
        createdAt: '2026-04-29T00:00:00.000Z',
        appVersion: packageJson.version,
        historyMode: 'light',
        scopes: {
          config: true,
          workspace: true,
          history: true,
          automation: true,
          analytics: true,
        },
        counts: {
          projects: 1,
          historyItems: 1,
          transcriptFiles: 1,
          summaryFiles: 1,
          automationRules: 1,
          automationProcessedEntries: 1,
          analyticsFiles: 1,
        },
      },
    });
    testContext.invokeMock.mockResolvedValue(undefined);

    const result = await uploadBackup({
      serverUrl: 'https://dav.example.com/root',
      remoteDir: 'backups/sona',
      username: 'demo',
      password: 'secret',
    });

    expect(testContext.exportBackupMock).toHaveBeenCalledWith({
      archivePath: '/temp/sona-webdav-backup-upload-123456-4fzzzxjy/sona-backup-test.tar.bz2',
    });
    expect(testContext.invokeMock).toHaveBeenCalledWith('webdav_upload_backup', {
      config: {
        serverUrl: 'https://dav.example.com/root',
        remoteDir: 'backups/sona',
        username: 'demo',
        password: 'secret',
      },
      localArchivePath: '/temp/sona-webdav-backup-upload-123456-4fzzzxjy/sona-backup-test.tar.bz2',
    });
    expect(result.fileName).toBe('sona-backup-test.tar.bz2');
    expect(testContext.removeMock).toHaveBeenCalledWith('/temp/sona-webdav-backup-upload-123456-4fzzzxjy', {
      recursive: true,
    });
  });

  it('downloads a remote archive, reuses local import preview, and returns the handle-shaped prepared import', async () => {
    const prepared = {
      importId: 'import-1',
      archivePath: '/downloads/remote-backup.tar.bz2',
      manifest: {
        schemaVersion: 1,
        createdAt: '2026-04-29T00:00:00.000Z',
        appVersion: packageJson.version,
        historyMode: 'light',
        scopes: {
          config: true,
          workspace: true,
          history: true,
          automation: true,
          analytics: true,
        },
        counts: {
          projects: 1,
          historyItems: 1,
          transcriptFiles: 1,
          summaryFiles: 1,
          automationRules: 1,
          automationProcessedEntries: 1,
          analyticsFiles: 1,
        },
      },
      config: {} as any,
      projects: [],
      automationRules: [],
      automationProcessedEntries: [],
      analyticsContent: '{}',
    };
    testContext.invokeMock.mockResolvedValue(undefined);
    testContext.prepareImportBackupMock.mockResolvedValue(prepared);

    const result = await prepareImportFromRemote(
      {
        href: 'https://dav.example.com/backups/remote-backup.tar.bz2',
        fileName: 'remote-backup.tar.bz2',
        size: 100,
        modifiedAt: '2026-04-29T00:00:00.000Z',
      },
      {
        serverUrl: 'https://dav.example.com/root',
        remoteDir: 'backups/sona',
        username: 'demo',
        password: 'secret',
      },
    );

    expect(testContext.invokeMock).toHaveBeenCalledWith('webdav_download_backup', {
      config: {
        serverUrl: 'https://dav.example.com/root',
        remoteDir: 'backups/sona',
        username: 'demo',
        password: 'secret',
      },
      href: 'https://dav.example.com/backups/remote-backup.tar.bz2',
      outputPath: '/temp/sona-webdav-backup-download-123456-4fzzzxjy/remote-backup.tar.bz2',
    });
    expect(testContext.prepareImportBackupMock).toHaveBeenCalledWith({
      archivePath: '/temp/sona-webdav-backup-download-123456-4fzzzxjy/remote-backup.tar.bz2',
    });
    expect(result).toBe(prepared);
    expect(testContext.removeMock).toHaveBeenCalledWith('/temp/sona-webdav-backup-download-123456-4fzzzxjy', {
      recursive: true,
    });
  });
});
