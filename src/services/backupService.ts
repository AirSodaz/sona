import packageJson from '../../package.json';
import { loadAutomationProcessedEntries, loadAutomationRules, saveAutomationProcessedEntries, saveAutomationRules } from './automation/automationService';
import { migrateConfig } from './configMigrationService';
import { historyService } from './historyService';
import { projectService } from './projectService';
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
import type { ProjectRecord } from '../types/project';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import {
  applyPreparedHistoryImport,
  disposePreparedBackupImport,
  exportBackupArchive,
  prepareBackupImport,
} from './tauri/backup';
import { llmUsageReadRaw, llmUsageReplaceRaw } from './tauri/llmUsage';
import { openDialog, saveDialog } from './tauri/platform/dialog';

const ANALYTICS_FALLBACK_CONTENT = '{}';

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
      projects: typeof counts.projects === 'number' ? counts.projects : 0,
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
  getConfig: () => AppConfig;
  setConfig: (config: AppConfig) => void;
  loadProjects: () => Promise<void>;
  loadHistoryItems: () => Promise<void>;
  getTranscriptSourceHistoryId: () => string | null;
  getHistoryItems: () => HistoryItem[];
  clearActiveTranscriptSession: (options: { clearAudio: boolean }) => void;
  openTranscriptSession: typeof openTranscriptSession;
  setAudioFile: (file: File | null) => void;
  loadAutomationRules: typeof loadAutomationRules;
  loadAutomationProcessedEntries: typeof loadAutomationProcessedEntries;
  saveAutomationRules: typeof saveAutomationRules;
  saveAutomationProcessedEntries: typeof saveAutomationProcessedEntries;
  migrateConfig: typeof migrateConfig;
  projectServiceGetAll: typeof projectService.getAll;
  projectServiceSaveAll: typeof projectService.saveAll;
  historyServiceLoadTranscript: typeof historyService.loadTranscript;
  historyServiceGetAudioUrl: typeof historyService.getAudioUrl;
  llmUsageReadRaw: typeof llmUsageReadRaw;
  llmUsageReplaceRaw: typeof llmUsageReplaceRaw;
  openDialog: typeof openDialog;
  saveDialog: typeof saveDialog;
  exportBackupArchive: typeof exportBackupArchive;
  prepareBackupImport: typeof prepareBackupImport;
  disposePreparedBackupImport: typeof disposePreparedBackupImport;
  applyPreparedHistoryImport: typeof applyPreparedHistoryImport;
  settingsStoreSet: (key: string, value: unknown) => Promise<void>;
  settingsStoreSave: () => Promise<void>;
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

  private async loadProjectsForBackup(config: AppConfig): Promise<ProjectRecord[]> {
    const fallbackEnabledPolishKeywordSetIds = (config.polishKeywordSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id);
    const fallbackEnabledSpeakerProfileIds = (config.speakerProfiles || [])
      .filter((profile) => profile.enabled)
      .map((profile) => profile.id);

    return this.ports.projectServiceGetAll({
      fallbackEnabledPolishKeywordSetIds,
      fallbackEnabledSpeakerProfileIds,
    });
  }

  private async loadAnalyticsContentForBackup(): Promise<string> {
    try {
      return await this.ports.llmUsageReadRaw();
    } catch {
      return ANALYTICS_FALLBACK_CONTENT;
    }
  }

  private async writeAnalyticsImport(analyticsContent: string): Promise<void> {
    await this.ports.llmUsageReplaceRaw(analyticsContent);
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

    const config = this.ports.getConfig();
    const [projects, automationRules, automationProcessedEntries, analyticsContent] = await Promise.all([
      this.loadProjectsForBackup(config),
      this.ports.loadAutomationRules(),
      this.ports.loadAutomationProcessedEntries(),
      this.loadAnalyticsContentForBackup(),
    ]);

    const manifest = await this.ports.exportBackupArchive<BackupManifestV1>({
      archivePath,
      appVersion: this.ports.appVersion,
      config,
      projects,
      automationRules,
      automationProcessedEntries,
      analyticsContent,
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
    this.ensureBackupOperationsIdle();

    let automationReloaded = false;
    await this.ports.stopAllAutomation();

    try {
      const migration = await this.ports.migrateConfig(prepared.config);
      const migratedConfig = migration.config;

      await this.ports.settingsStoreSet(STORE_KEY_CONFIG, migratedConfig);
      await this.ports.settingsStoreSave();
      this.ports.setConfig(migratedConfig);

      await this.ports.projectServiceSaveAll(prepared.projects);
      await this.ports.saveAutomationRules(prepared.automationRules);
      await this.ports.saveAutomationProcessedEntries(prepared.automationProcessedEntries);
      await this.writeAnalyticsImport(prepared.analyticsContent);
      await this.ports.applyPreparedHistoryImport(prepared.importId);

      await this.ports.loadProjects();
      await this.ports.loadHistoryItems();
      await this.ports.loadAndStartAutomation();
      automationReloaded = true;
      await this.syncOpenTranscriptAfterImport();
    } finally {
      if (!automationReloaded) {
        await this.ports.loadAndStartAutomation().catch((error) => {
          logger.error('[Backup] Failed to restart automation after import error:', error);
        });
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
  getConfig: () => useConfigStore.getState().config,
  setConfig: (config) => useConfigStore.setState({ config }),
  loadProjects: () => useProjectStore.getState().loadProjects(),
  loadHistoryItems: () => useHistoryStore.getState().loadItems(),
  getTranscriptSourceHistoryId: () => useTranscriptSessionStore.getState().sourceHistoryId,
  getHistoryItems: () => useHistoryStore.getState().items,
  clearActiveTranscriptSession,
  openTranscriptSession,
  setAudioFile: (file) => useTranscriptPlaybackStore.getState().setAudioFile(file),
  loadAutomationRules,
  loadAutomationProcessedEntries,
  saveAutomationRules,
  saveAutomationProcessedEntries,
  migrateConfig,
  projectServiceGetAll: (options) => projectService.getAll(options),
  projectServiceSaveAll: (projects) => projectService.saveAll(projects),
  historyServiceLoadTranscript: (historyId) => historyService.loadTranscript(historyId),
  historyServiceGetAudioUrl: (historyId) => historyService.getAudioUrl(historyId),
  llmUsageReadRaw,
  llmUsageReplaceRaw,
  openDialog,
  saveDialog,
  exportBackupArchive,
  prepareBackupImport,
  disposePreparedBackupImport,
  applyPreparedHistoryImport,
  settingsStoreSet: (key, value) => settingsStore.set(key, value),
  settingsStoreSave: () => settingsStore.save(),
  appVersion: packageJson.version,
});

// Re-export methods used by standalone imports to minimize consumer breakage
export const buildDefaultBackupFileName = backupService.buildDefaultBackupFileName.bind(backupService);
export const getBackupOperationBlocker = backupService.getBackupOperationBlocker.bind(backupService);
export const exportBackup = backupService.exportBackup.bind(backupService);
export const prepareImportBackup = backupService.prepareImportBackup.bind(backupService);
export const disposePreparedImport = backupService.disposePreparedImport.bind(backupService);
export const applyImportBackup = backupService.applyImportBackup.bind(backupService);
