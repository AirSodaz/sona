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

export function buildDefaultBackupFileName(date = new Date()): string {
  return `sona-backup-${formatBackupTimestamp(date)}.tar.bz2`;
}

export function getBackupOperationBlocker(): BackupOperationBlocker | null {
  if (useTranscriptRuntimeStore.getState().isRecording) {
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

async function pickExportArchivePath(): Promise<string | null> {
  const selected = await saveDialog({
    defaultPath: buildDefaultBackupFileName(),
  });

  return typeof selected === 'string' && selected.trim().length > 0 ? selected : null;
}

async function pickImportArchivePath(): Promise<string | null> {
  const selected = await openDialog({
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
  try {
    return await llmUsageReadRaw();
  } catch {
    return ANALYTICS_FALLBACK_CONTENT;
  }
}

async function writeAnalyticsImport(analyticsContent: string): Promise<void> {
  await llmUsageReplaceRaw(analyticsContent);
}

async function syncOpenTranscriptAfterImport(): Promise<void> {
  const transcriptSession = useTranscriptSessionStore.getState();
  const currentHistoryId = transcriptSession.sourceHistoryId;

  if (!currentHistoryId) {
    return;
  }

  const historyItems = useHistoryStore.getState().items;
  const matchingItem = historyItems.find((item) => item.id === currentHistoryId);

  if (!matchingItem) {
    clearActiveTranscriptSession({ clearAudio: true });
    return;
  }

  const segments = await historyService.loadTranscript(matchingItem.transcriptPath);
  if (!segments) {
    clearActiveTranscriptSession({ clearAudio: true });
    return;
  }

  openTranscriptSession({
    segments,
    sourceHistoryId: matchingItem.id,
    title: matchingItem.title,
    icon: matchingItem.icon || null,
    audioUrl: await historyService.getAudioUrl(matchingItem.audioPath),
  });
  useTranscriptPlaybackStore.getState().setAudioFile(null);
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

  const manifest = await exportBackupArchive<BackupManifestV1>({
    archivePath,
    appVersion: packageJson.version,
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

export async function prepareImportBackup(options?: {
  archivePath?: string;
}): Promise<PreparedBackupImport | null> {
  ensureBackupOperationsIdle();

  const archivePath = options?.archivePath || await pickImportArchivePath();
  if (!archivePath) {
    return null;
  }

  return prepareBackupImport<PreparedBackupImport>(archivePath);
}

export async function disposePreparedImport(prepared: PreparedBackupImport): Promise<void> {
  await disposePreparedBackupImport(prepared.importId);
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
    await applyPreparedHistoryImport(prepared.importId);

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
