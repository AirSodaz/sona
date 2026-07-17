import packageJson from '../../package.json';
import { historyService } from './historyService';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useConfigStore } from '../stores/configStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { clearActiveTranscriptSession, openTranscriptSession } from '../stores/transcriptCoordinator';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import {
  BACKUP_HISTORY_MODE,
  BACKUP_SCHEMA_VERSION,
  type BackupManifestV1,
  type BackupOperationBlocker,
  type ExportBackupResult,
  type PreparedBackupImport,
} from '../types/backup';
import type { AppConfig } from '../types/config';
import type { HistoryItem } from '../types/history';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import {
  applyPreparedHistoryImport,
  disposePreparedBackupImport,
  exportBackupArchive,
  prepareBackupImport,
} from './tauri/backup';
import { openDialog, saveDialog } from './tauri/platform/dialog';

class BackupOperationBlockedError extends Error {
  constructor(public readonly blocker: BackupOperationBlocker, message: string) {
    super(message);
    this.name = 'BackupOperationBlockedError';
  }
}

function padNumber(value: number): string {
  return String(value).padStart(2, '0');
}

function formatBackupTimestamp(date = new Date()): string {
  return [
    date.getFullYear(),
    padNumber(date.getMonth() + 1),
    padNumber(date.getDate()),
  ].join('-') + '_' + [
    padNumber(date.getHours()),
    padNumber(date.getMinutes()),
    padNumber(date.getSeconds()),
  ].join('-');
}

function ensureRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }

  return value as Record<string, unknown>;
}

function validateManifest(raw: unknown): BackupManifestV1 {
  const source = ensureRecord(raw, 'Backup manifest');
  const scopes = ensureRecord(source.scopes, 'Backup manifest scopes');
  const counts = ensureRecord(source.counts, 'Backup manifest counts');

  if (source.schemaVersion !== BACKUP_SCHEMA_VERSION) {
    throw new Error(`Unsupported backup schema version: ${String(source.schemaVersion)}`);
  }

  if (source.historyMode !== BACKUP_HISTORY_MODE) {
    throw new Error(`Unsupported backup history mode: ${String(source.historyMode)}`);
  }

  const requiredScopes = ['config', 'workspace', 'history', 'automation', 'analytics'] as const;
  requiredScopes.forEach((scope) => {
    if (scopes[scope] !== true) {
      throw new Error(`Backup manifest is missing the required "${scope}" scope.`);
    }
  });

  return {
    schemaVersion: BACKUP_SCHEMA_VERSION,
    createdAt: typeof source.createdAt === 'string' ? source.createdAt : new Date(0).toISOString(),
    appVersion: typeof source.appVersion === 'string' ? source.appVersion : 'unknown',
    historyMode: BACKUP_HISTORY_MODE,
    scopes: {
      config: true,
      workspace: true,
      history: true,
      automation: true,
      analytics: true,
    },
    counts: {
      tags: typeof counts.tags === 'number' ? counts.tags : 0,
      historyItems: typeof counts.historyItems === 'number' ? counts.historyItems : 0,
      transcriptFiles: typeof counts.transcriptFiles === 'number' ? counts.transcriptFiles : 0,
      summaryFiles: typeof counts.summaryFiles === 'number' ? counts.summaryFiles : 0,
      automationRules: typeof counts.automationRules === 'number' ? counts.automationRules : 0,
      automationProcessedEntries: typeof counts.automationProcessedEntries === 'number' ? counts.automationProcessedEntries : 0,
      analyticsFiles: typeof counts.analyticsFiles === 'number' ? counts.analyticsFiles : 0,
    },
  };
}

export interface BackupServicePorts {
  getIsRecording: () => boolean;
  getHasBlockingQueueItems: () => boolean;
  stopAllAutomation: () => Promise<void>;
  loadAndStartAutomation: () => Promise<void>;
  reloadConfig: () => Promise<void>;
  loadProjects: () => Promise<void>;
  getProjectLoadError: () => string | null;
  loadHistoryItems: () => Promise<void>;
  getHistoryLoadError: () => string | null;
  getTranscriptSourceHistoryId: () => string | null;
  getHistoryItems: () => HistoryItem[];
  clearActiveTranscriptSession: (options: { clearAudio: boolean }) => void;
  openTranscriptSession: typeof openTranscriptSession;
  setAudioFile: (file: File | null) => void;
  historyServiceLoadTranscript: typeof historyService.loadTranscript;
  historyServiceGetAudioUrl: typeof historyService.getAudioUrl;
  openDialog: typeof openDialog;
  saveDialog: typeof saveDialog;
  exportBackupArchive: typeof exportBackupArchive;
  prepareBackupImport: typeof prepareBackupImport;
  disposePreparedBackupImport: typeof disposePreparedBackupImport;
  applyPreparedHistoryImport: typeof applyPreparedHistoryImport;
  appVersion: string;
}

export class BackupService {
  constructor(private readonly ports: BackupServicePorts) {}

  buildDefaultBackupFileName(date = new Date()): string {
    return `sona-backup-${formatBackupTimestamp(date)}.tar.bz2`;
  }

  getBackupOperationBlocker(): BackupOperationBlocker | null {
    if (this.ports.getIsRecording()) {
      return 'recording';
    }

    if (this.ports.getHasBlockingQueueItems()) {
      return 'batch_queue';
    }

    return null;
  }

  private ensureBackupOperationsIdle(): void {
    const blocker = this.getBackupOperationBlocker();
    if (!blocker) {
      return;
    }

    if (blocker === 'recording') {
      throw new BackupOperationBlockedError(
        blocker,
        'Stop Live Record before exporting or importing backups.',
      );
    }

    throw new BackupOperationBlockedError(
      blocker,
      'Wait for Batch Import to finish or clear pending items before exporting or importing backups.',
    );
  }

  private async pickExportArchivePath(): Promise<string | null> {
    const selected = await this.ports.saveDialog({
      defaultPath: this.buildDefaultBackupFileName(),
    });

    return typeof selected === 'string' && selected.trim().length > 0 ? selected : null;
  }

  private async pickImportArchivePath(): Promise<string | null> {
    const selected = await this.ports.openDialog({
      multiple: false,
    });

    return typeof selected === 'string' && selected.trim().length > 0 ? selected : null;
  }

  private async syncOpenTranscriptAfterImport(): Promise<void> {
    const currentHistoryId = this.ports.getTranscriptSourceHistoryId();

    if (!currentHistoryId) {
      return;
    }

    const historyItems = this.ports.getHistoryItems();
    const matchingItem = historyItems.find(item => item.id === currentHistoryId);

    if (!matchingItem) {
      this.ports.clearActiveTranscriptSession({ clearAudio: true });
      return;
    }

    const segments = await this.ports.historyServiceLoadTranscript(matchingItem.id);
    if (!segments) {
      this.ports.clearActiveTranscriptSession({ clearAudio: true });
      return;
    }

    this.ports.openTranscriptSession({
      segments,
      sourceHistoryId: matchingItem.id,
      title: matchingItem.title,
      icon: matchingItem.icon || null,
      audioUrl: await this.ports.historyServiceGetAudioUrl(matchingItem.id),
    });
    this.ports.setAudioFile(null);
  }

  async exportBackup(options?: {
    archivePath?: string;
  }): Promise<ExportBackupResult | null> {
    this.ensureBackupOperationsIdle();

    const archivePath = options?.archivePath || await this.pickExportArchivePath();
    if (!archivePath) {
      return null;
    }

    const manifest = await this.ports.exportBackupArchive<BackupManifestV1>({
      archivePath,
      appVersion: this.ports.appVersion,
    });

    return {
      archivePath,
      manifest: validateManifest(manifest),
    };
  }

  async prepareImportBackup(options?: {
    archivePath?: string;
  }): Promise<PreparedBackupImport | null> {
    this.ensureBackupOperationsIdle();

    const archivePath = options?.archivePath || await this.pickImportArchivePath();
    if (!archivePath) {
      return null;
    }

    return this.ports.prepareBackupImport<PreparedBackupImport>(archivePath);
  }

  async disposePreparedImport(prepared: PreparedBackupImport): Promise<void> {
    await this.ports.disposePreparedBackupImport(prepared.importId);
  }

  async applyImportBackup(prepared: PreparedBackupImport): Promise<void> {
    let backendApplied = false;
    let automationStopAttempted = false;
    let automationReloaded = false;
    let transcriptSynchronized = false;
    try {
      this.ensureBackupOperationsIdle();
      automationStopAttempted = true;
      await this.ports.stopAllAutomation();
      await this.ports.applyPreparedHistoryImport(prepared.importId);
      backendApplied = true;

      await this.ports.reloadConfig();
      let projectReloadError: Error | null = null;
      try {
        await this.ports.loadProjects();
        const error = this.ports.getProjectLoadError();
        if (error) throw new Error(`Failed to reload projects after backup restore: ${error}`);
      } catch (error) {
        projectReloadError = error instanceof Error ? error : new Error(extractErrorMessage(error));
      }

      let historyReloadError: Error | null = null;
      try {
        await this.ports.loadHistoryItems();
        const error = this.ports.getHistoryLoadError();
        if (error) throw new Error(`Failed to reload history after backup restore: ${error}`);
      } catch (error) {
        historyReloadError = error instanceof Error ? error : new Error(extractErrorMessage(error));
      }

      if (projectReloadError) throw projectReloadError;
      if (historyReloadError) throw historyReloadError;

      await this.ports.loadAndStartAutomation();
      automationReloaded = true;
      await this.syncOpenTranscriptAfterImport();
      transcriptSynchronized = true;
    } finally {
      if (automationStopAttempted && !automationReloaded) {
        await this.ports.loadAndStartAutomation().catch((error) => {
          logger.error('[Backup] Failed to restart automation after import error:', error);
        });
      }

      if (backendApplied && !transcriptSynchronized) {
        try {
          this.ports.clearActiveTranscriptSession({ clearAudio: true });
        } catch (error) {
          logger.error(
            '[Backup] Failed to clear transcript after import error:',
            extractErrorMessage(error),
          );
        }
      }

      await this.disposePreparedImport(prepared).catch((error) => {
        logger.error('[Backup] Failed to dispose prepared backup import:', extractErrorMessage(error));
      });
    }
  }
}

export function createBackupService(ports: BackupServicePorts): BackupService {
  return new BackupService(ports);
}

export const backupService = createBackupService({
  getIsRecording: () => useTranscriptRuntimeStore.getState().isRecording,
  getHasBlockingQueueItems: () => useBatchQueueStore.getState().queueItems.some((item) => (
    item.status === 'pending' || item.status === 'processing'
  )),
  stopAllAutomation: () => useAutomationStore.getState().stopAll(),
  loadAndStartAutomation: () => useAutomationStore.getState().loadAndStart(),
  reloadConfig: async () => {
    const config = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
    if (!config) throw new Error('Restored backup config is missing.');
    useConfigStore.getState().setConfig(config);
    await settingsStore.notifyExternalUpdate(STORE_KEY_CONFIG, config);
  },
  loadProjects: () => useProjectStore.getState().loadProjects(),
  getProjectLoadError: () => useProjectStore.getState().error,
  loadHistoryItems: () => useHistoryStore.getState().loadItems(),
  getHistoryLoadError: () => useHistoryStore.getState().error,
  getTranscriptSourceHistoryId: () => useTranscriptSessionStore.getState().sourceHistoryId,
  getHistoryItems: () => useHistoryStore.getState().items,
  clearActiveTranscriptSession,
  openTranscriptSession,
  setAudioFile: (file) => useTranscriptPlaybackStore.getState().setAudioFile(file),
  historyServiceLoadTranscript: (historyId) => historyService.loadTranscript(historyId),
  historyServiceGetAudioUrl: (historyId) => historyService.getAudioUrl(historyId),
  openDialog,
  saveDialog,
  exportBackupArchive,
  prepareBackupImport,
  disposePreparedBackupImport,
  applyPreparedHistoryImport,
  appVersion: packageJson.version,
});

// Re-export methods used by standalone imports to minimize consumer breakage
export const buildDefaultBackupFileName = backupService.buildDefaultBackupFileName.bind(backupService);
export const getBackupOperationBlocker = backupService.getBackupOperationBlocker.bind(backupService);
export const exportBackup = backupService.exportBackup.bind(backupService);
export const prepareImportBackup = backupService.prepareImportBackup.bind(backupService);
export const disposePreparedImport = backupService.disposePreparedImport.bind(backupService);
export const applyImportBackup = backupService.applyImportBackup.bind(backupService);
