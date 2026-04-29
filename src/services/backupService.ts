import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { BaseDirectory, mkdir, readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import packageJson from '../../package.json';
import { loadAutomationProcessedEntries, loadAutomationRules, saveAutomationProcessedEntries, saveAutomationRules } from './automationService';
import { migrateConfig } from './configMigrationService';
import { historyService } from './historyService';
import { llmUsageService } from './llmUsageService';
import { projectService } from './projectService';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useConfigStore } from '../stores/configStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AutomationProcessedEntry, AutomationRule } from '../types/automation';
import {
  BACKUP_HISTORY_MODE,
  BACKUP_SCHEMA_VERSION,
  type BackupManifestV1,
  type BackupOperationBlocker,
  type ExportBackupResult,
  type PreparedBackupImport,
} from '../types/backup';
import type { AppConfig } from '../types/config';
import { normalizeProjectRecord } from '../types/project';
import type { ProjectRecord } from '../types/project';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';

const APP_LOCAL_ANALYTICS_DIR = 'analytics';
const APP_LOCAL_ANALYTICS_USAGE_FILE = `${APP_LOCAL_ANALYTICS_DIR}/llm-usage.json`;
const ANALYTICS_FALLBACK_CONTENT = '{}';

class BackupOperationBlockedError extends Error {
  constructor(public readonly blocker: BackupOperationBlocker, message: string) {
    super(message);
    this.name = 'BackupOperationBlockedError';
  }
}

interface PreparedBackupImportPayload {
  importId: string;
  archivePath: string;
  manifest: BackupManifestV1;
  config: unknown;
  projects: unknown[];
  automationRules: unknown[];
  automationProcessedEntries: unknown[];
  analyticsContent: string;
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

export function buildDefaultBackupFileName(date = new Date()): string {
  return `sona-backup-${formatBackupTimestamp(date)}.tar.bz2`;
}

export function getBackupOperationBlocker(): BackupOperationBlocker | null {
  if (useTranscriptStore.getState().isRecording) {
    return 'recording';
  }

  const hasBlockingQueueItems = useBatchQueueStore.getState().queueItems.some((item) => (
    item.status === 'pending' || item.status === 'processing'
  ));

  return hasBlockingQueueItems ? 'batch_queue' : null;
}

function ensureBackupOperationsIdle(): void {
  const blocker = getBackupOperationBlocker();
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

function ensureRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }

  return value as Record<string, unknown>;
}

function ensureArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array.`);
  }

  return value;
}

function normalizeAutomationRule(input: unknown): AutomationRule {
  const source = ensureRecord(input, 'Automation rule');

  return {
    id: typeof source.id === 'string' ? source.id : '',
    name: typeof source.name === 'string' ? source.name : '',
    projectId: typeof source.projectId === 'string' ? source.projectId : '',
    presetId: source.presetId as AutomationRule['presetId'],
    watchDirectory: typeof source.watchDirectory === 'string' ? source.watchDirectory : '',
    recursive: Boolean(source.recursive),
    enabled: Boolean(source.enabled),
    stageConfig: ensureRecord(source.stageConfig ?? {}, 'Automation stage config') as unknown as AutomationRule['stageConfig'],
    exportConfig: ensureRecord(source.exportConfig ?? {}, 'Automation export config') as unknown as AutomationRule['exportConfig'],
    createdAt: typeof source.createdAt === 'number' && Number.isFinite(source.createdAt) ? source.createdAt : 0,
    updatedAt: typeof source.updatedAt === 'number' && Number.isFinite(source.updatedAt) ? source.updatedAt : 0,
  };
}

function normalizeAutomationProcessedEntry(input: unknown): AutomationProcessedEntry {
  const source = ensureRecord(input, 'Automation processed entry');

  return {
    ruleId: typeof source.ruleId === 'string' ? source.ruleId : '',
    filePath: typeof source.filePath === 'string' ? source.filePath : '',
    sourceFingerprint: typeof source.sourceFingerprint === 'string' ? source.sourceFingerprint : '',
    size: typeof source.size === 'number' && Number.isFinite(source.size) ? source.size : 0,
    mtimeMs: typeof source.mtimeMs === 'number' && Number.isFinite(source.mtimeMs) ? source.mtimeMs : 0,
    status: source.status === 'error' || source.status === 'discarded' ? source.status : 'complete',
    processedAt: typeof source.processedAt === 'number' && Number.isFinite(source.processedAt) ? source.processedAt : 0,
    historyId: typeof source.historyId === 'string' ? source.historyId : undefined,
    exportPath: typeof source.exportPath === 'string' ? source.exportPath : undefined,
    errorMessage: typeof source.errorMessage === 'string' ? source.errorMessage : undefined,
  };
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

async function ensureAppLocalDataDirectory(path: string): Promise<void> {
  await mkdir(path, { baseDir: BaseDirectory.AppLocalData, recursive: true });
}

async function pickExportArchivePath(): Promise<string | null> {
  const selected = await save({
    defaultPath: buildDefaultBackupFileName(),
  });

  return typeof selected === 'string' && selected.trim().length > 0 ? selected : null;
}

async function pickImportArchivePath(): Promise<string | null> {
  const selected = await open({
    multiple: false,
  });

  return typeof selected === 'string' && selected.trim().length > 0 ? selected : null;
}

async function loadProjectsForBackup(config: AppConfig): Promise<ProjectRecord[]> {
  const fallbackEnabledPolishKeywordSetIds = (config.polishKeywordSets || [])
    .filter((set) => set.enabled)
    .map((set) => set.id);
  const fallbackEnabledSpeakerProfileIds = (config.speakerProfiles || [])
    .filter((profile) => profile.enabled)
    .map((profile) => profile.id);

  return projectService.getAll({
    fallbackEnabledPolishKeywordSetIds,
    fallbackEnabledSpeakerProfileIds,
  });
}

async function loadAnalyticsContentForBackup(): Promise<string> {
  await llmUsageService.init();

  try {
    return await readTextFile(APP_LOCAL_ANALYTICS_USAGE_FILE, {
      baseDir: BaseDirectory.AppLocalData,
    });
  } catch {
    return ANALYTICS_FALLBACK_CONTENT;
  }
}

async function writeAnalyticsImport(analyticsContent: string): Promise<void> {
  await ensureAppLocalDataDirectory(APP_LOCAL_ANALYTICS_DIR);
  await writeTextFile(APP_LOCAL_ANALYTICS_USAGE_FILE, analyticsContent, {
    baseDir: BaseDirectory.AppLocalData,
  });
}

function normalizePreparedImport(raw: PreparedBackupImportPayload): PreparedBackupImport {
  const analyticsJson = JSON.parse(raw.analyticsContent) as unknown;
  ensureRecord(analyticsJson, 'Backup analytics');

  return {
    importId: typeof raw.importId === 'string' ? raw.importId : '',
    archivePath: typeof raw.archivePath === 'string' ? raw.archivePath : '',
    manifest: validateManifest(raw.manifest),
    config: ensureRecord(raw.config, 'Backup config') as unknown as AppConfig,
    projects: ensureArray(raw.projects, 'Backup projects').map((item) => (
      normalizeProjectRecord(item as Partial<ProjectRecord>)
    )),
    automationRules: ensureArray(raw.automationRules, 'Backup automation rules').map((item) => (
      normalizeAutomationRule(item)
    )),
    automationProcessedEntries: ensureArray(raw.automationProcessedEntries, 'Backup automation processed entries').map((item) => (
      normalizeAutomationProcessedEntry(item)
    )),
    analyticsContent: raw.analyticsContent,
  };
}

async function syncOpenTranscriptAfterImport(): Promise<void> {
  const transcriptStore = useTranscriptStore.getState();
  const currentHistoryId = transcriptStore.sourceHistoryId;

  if (!currentHistoryId) {
    return;
  }

  const historyItems = useHistoryStore.getState().items;
  const matchingItem = historyItems.find((item) => item.id === currentHistoryId);

  if (!matchingItem) {
    transcriptStore.clearSegments();
    transcriptStore.setAudioFile(null);
    transcriptStore.setAudioUrl(null);
    return;
  }

  const segments = await historyService.loadTranscript(matchingItem.transcriptPath);
  if (!segments) {
    transcriptStore.clearSegments();
    transcriptStore.setAudioFile(null);
    transcriptStore.setAudioUrl(null);
    return;
  }

  transcriptStore.loadTranscript(segments, matchingItem.id, matchingItem.title, matchingItem.icon || null);
  transcriptStore.setAudioFile(null);
  transcriptStore.setAudioUrl(await historyService.getAudioUrl(matchingItem.audioPath));
}

export async function exportBackup(options?: {
  archivePath?: string;
}): Promise<ExportBackupResult | null> {
  ensureBackupOperationsIdle();

  const archivePath = options?.archivePath || await pickExportArchivePath();
  if (!archivePath) {
    return null;
  }

  const config = useConfigStore.getState().config;
  const [projects, automationRules, automationProcessedEntries, analyticsContent] = await Promise.all([
    loadProjectsForBackup(config),
    loadAutomationRules(),
    loadAutomationProcessedEntries(),
    loadAnalyticsContentForBackup(),
  ]);

  const manifest = await invoke<BackupManifestV1>('export_backup_archive', {
    request: {
      archivePath,
      appVersion: packageJson.version,
      config,
      projects,
      automationRules,
      automationProcessedEntries,
      analyticsContent,
    },
  });

  return {
    archivePath,
    manifest: validateManifest(manifest),
  };
}

export async function prepareImportBackup(options?: {
  archivePath?: string;
}): Promise<PreparedBackupImport | null> {
  ensureBackupOperationsIdle();

  const archivePath = options?.archivePath || await pickImportArchivePath();
  if (!archivePath) {
    return null;
  }

  const prepared = await invoke<PreparedBackupImportPayload>('prepare_backup_import', {
    archivePath,
  });
  return normalizePreparedImport(prepared);
}

export async function disposePreparedImport(prepared: PreparedBackupImport): Promise<void> {
  await invoke('dispose_prepared_backup_import', { importId: prepared.importId });
}

export async function applyImportBackup(prepared: PreparedBackupImport): Promise<void> {
  ensureBackupOperationsIdle();

  let automationReloaded = false;
  await useAutomationStore.getState().stopAll();

  try {
    const migration = await migrateConfig(prepared.config);
    const migratedConfig = migration.config;

    await settingsStore.set(STORE_KEY_CONFIG, migratedConfig);
    await settingsStore.save();
    useConfigStore.setState({ config: migratedConfig });

    await projectService.saveAll(prepared.projects);
    await saveAutomationRules(prepared.automationRules);
    await saveAutomationProcessedEntries(prepared.automationProcessedEntries);
    await writeAnalyticsImport(prepared.analyticsContent);
    await invoke('apply_prepared_history_import', { importId: prepared.importId });

    await useProjectStore.getState().loadProjects();
    await useHistoryStore.getState().loadItems();
    await useAutomationStore.getState().loadAndStart();
    automationReloaded = true;
    await syncOpenTranscriptAfterImport();
  } finally {
    if (!automationReloaded) {
      await useAutomationStore.getState().loadAndStart().catch((error) => {
        logger.error('[Backup] Failed to restart automation after import error:', error);
      });
    }

    await disposePreparedImport(prepared).catch((error) => {
      logger.error('[Backup] Failed to dispose prepared backup import:', extractErrorMessage(error));
    });
  }
}

export const backupService = {
  buildDefaultBackupFileName,
  getBackupOperationBlocker,
  exportBackup,
  prepareImportBackup,
  disposePreparedImport,
  applyImportBackup,
};
