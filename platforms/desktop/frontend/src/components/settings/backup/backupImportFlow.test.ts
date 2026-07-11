import { describe, expect, it, vi } from 'vitest';
import type {
  BackupManifestV1,
  PreparedBackupImport,
} from '../../../types/backup';
import { runPreparedBackupImportFlow } from './backupImportFlow';

function buildManifest(): BackupManifestV1 {
  return {
    schemaVersion: 1,
    createdAt: '2026-05-01T00:00:00.000Z',
    appVersion: '0.6.4',
    historyMode: 'light',
    scopes: {
      config: true,
      workspace: true,
      history: true,
      automation: true,
      analytics: true,
    },
    counts: {
      projects: 2,
      historyItems: 5,
      transcriptFiles: 5,
      summaryFiles: 3,
      automationRules: 1,
      automationProcessedEntries: 7,
      analyticsFiles: 1,
    },
  };
}

function buildPreparedImport(): PreparedBackupImport {
  return {
    importId: 'import-flow',
    archivePath: 'C:\\backups\\sona-backup.tar.bz2',
    manifest: buildManifest(),
    config: {} as unknown as PreparedBackupImport['config'],
    projects: [],
    automationRules: [],
    automationProcessedEntries: [],
    analyticsContent: '{}',
  } as PreparedBackupImport;
}

describe('runPreparedBackupImportFlow', () => {
  it('disposes a prepared import when the user cancels before apply', async () => {
    const prepared = buildPreparedImport();
    const prepare = vi.fn().mockResolvedValue(prepared);
    const confirm = vi.fn().mockResolvedValue(false);
    const apply = vi.fn();
    const dispose = vi.fn().mockResolvedValue(undefined);
    const alertSuccess = vi.fn();
    const onError = vi.fn();

    await runPreparedBackupImportFlow({
      prepare,
      confirm,
      apply,
      dispose,
      alertSuccess,
      onError,
    });

    expect(confirm).toHaveBeenCalledWith(prepared);
    expect(dispose).toHaveBeenCalledWith(prepared);
    expect(apply).not.toHaveBeenCalled();
    expect(alertSuccess).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('disposes a prepared import and reports the failure when apply throws', async () => {
    const prepared = buildPreparedImport();
    const failure = new Error('apply failed');
    const prepare = vi.fn().mockResolvedValue(prepared);
    const confirm = vi.fn().mockResolvedValue(true);
    const apply = vi.fn().mockRejectedValue(failure);
    const dispose = vi.fn().mockResolvedValue(undefined);
    const alertSuccess = vi.fn();
    const onError = vi.fn().mockResolvedValue(undefined);

    await runPreparedBackupImportFlow({
      prepare,
      confirm,
      apply,
      dispose,
      alertSuccess,
      onError,
    });

    expect(apply).toHaveBeenCalledWith(prepared);
    expect(dispose).toHaveBeenCalledWith(prepared);
    expect(alertSuccess).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(failure);
  });
});
