import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  backupService,
  type BackupServicePorts,
} from '../../../services/backupService';
import type {
  BackupManifestV1,
  PreparedBackupImport,
} from '../../../types/backup';
import { runPreparedBackupImportFlow } from './backupImportFlow';
import { preparedBackupImportActions } from './useBackupSettingsController';

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

describe('production prepared-backup callbacks', () => {
  const mutableService = backupService as unknown as { ports: BackupServicePorts };
  let originalPorts: BackupServicePorts;

  beforeEach(() => {
    originalPorts = mutableService.ports;
  });

  afterEach(() => {
    mutableService.ports = originalPorts;
  });

  function installPorts() {
    const applyPreparedHistoryImport = vi.fn().mockResolvedValue(undefined);
    const disposePreparedBackupImport = vi.fn().mockResolvedValue(undefined);
    mutableService.ports = {
      getIsRecording: () => false,
      getHasBlockingQueueItems: () => false,
      stopAllAutomation: vi.fn().mockResolvedValue(undefined),
      loadAndStartAutomation: vi.fn().mockResolvedValue(undefined),
      reloadConfig: vi.fn().mockResolvedValue(undefined),
      loadProjects: vi.fn().mockResolvedValue(undefined),
      getProjectLoadError: () => null,
      loadHistoryItems: vi.fn().mockResolvedValue(undefined),
      getHistoryLoadError: () => null,
      getTranscriptSourceHistoryId: () => null,
      getHistoryItems: () => [],
      clearActiveTranscriptSession: vi.fn(),
      openTranscriptSession: vi.fn(),
      setAudioFile: vi.fn(),
      historyServiceLoadTranscript: vi.fn(),
      historyServiceGetAudioUrl: vi.fn(),
      openDialog: vi.fn(),
      saveDialog: vi.fn(),
      exportBackupArchive: vi.fn(),
      prepareBackupImport: vi.fn(),
      disposePreparedBackupImport,
      applyPreparedHistoryImport,
      appVersion: '0.8.0',
    } as unknown as BackupServicePorts;
    return { applyPreparedHistoryImport, disposePreparedBackupImport };
  }

  it('reaches the backend apply through the controller-bound callback', async () => {
    const prepared = buildPreparedImport();
    const transports = installPorts();
    const alertSuccess = vi.fn().mockResolvedValue(undefined);

    await runPreparedBackupImportFlow({
      prepare: vi.fn().mockResolvedValue(prepared),
      confirm: vi.fn().mockResolvedValue(true),
      ...preparedBackupImportActions,
      alertSuccess,
      onError: vi.fn().mockResolvedValue(undefined),
    });

    expect(transports.applyPreparedHistoryImport).toHaveBeenCalledWith(prepared.importId);
    expect(transports.disposePreparedBackupImport).toHaveBeenCalledWith(prepared.importId);
    expect(alertSuccess).toHaveBeenCalledTimes(1);
  });

  it('disposes cancellation through the controller-bound callback', async () => {
    const prepared = buildPreparedImport();
    const transports = installPorts();

    await runPreparedBackupImportFlow({
      prepare: vi.fn().mockResolvedValue(prepared),
      confirm: vi.fn().mockResolvedValue(false),
      ...preparedBackupImportActions,
      alertSuccess: vi.fn().mockResolvedValue(undefined),
      onError: vi.fn().mockResolvedValue(undefined),
    });

    expect(transports.applyPreparedHistoryImport).not.toHaveBeenCalled();
    expect(transports.disposePreparedBackupImport).toHaveBeenCalledWith(prepared.importId);
  });
});
