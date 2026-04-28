import { invoke } from '@tauri-apps/api/core';
import { join, tempDir } from '@tauri-apps/api/path';
import { open, save } from '@tauri-apps/plugin-dialog';
import { BaseDirectory, exists, mkdir, readTextFile, remove, writeTextFile } from '@tauri-apps/plugin-fs';
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
import { BACKUP_HISTORY_MODE, BACKUP_SCHEMA_VERSION, type BackupManifestV1, type BackupOperationBlocker, type ExportBackupResult, type PreparedBackupImport } from '../types/backup';
import type { AppConfig } from '../types/config';
import type { HistoryItem } from '../types/history';
import { isHistoryItemDraft } from '../types/history';
import { normalizeProjectRecord } from '../types/project';
import type { ProjectRecord } from '../types/project';
import type { HistorySummaryPayload, TranscriptSegment } from '../types/transcript';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';

const CONFIG_DIR_NAME = 'config';
const CONFIG_FILE_NAME = 'sona-config.json';
const PROJECTS_DIR_NAME = 'projects';
const PROJECTS_INDEX_FILE = 'index.json';
const HISTORY_DIR_NAME = 'history';
const AUTOMATION_DIR_NAME = 'automation';
const AUTOMATION_RULES_FILE = 'rules.json';
const AUTOMATION_PROCESSED_FILE = 'processed.json';
const ANALYTICS_DIR_NAME = 'analytics';
const ANALYTICS_USAGE_FILE = 'llm-usage.json';

const APP_LOCAL_HISTORY_DIR = 'history';
const APP_LOCAL_ANALYTICS_DIR = 'analytics';
const APP_LOCAL_ANALYTICS_USAGE_FILE = `${APP_LOCAL_ANALYTICS_DIR}/${ANALYTICS_USAGE_FILE}`;

const SUMMARY_FILE_SUFFIX = '.summary.json';
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

function isSafeBackupFileName(value: string): boolean {
  return value.length > 0 && !value.includes('..') && !/[\\/]/.test(value);
}

function requireSafeBackupFileName(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new Error(`${label} must be a string.`);
  }

  const trimmed = value.trim();
  if (!isSafeBackupFileName(trimmed)) {
    throw new Error(`${label} contains an invalid file name.`);
  }

  return trimmed;
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

function normalizeHistoryItemForImport(input: unknown): HistoryItem {
  const source = ensureRecord(input, 'History item');
  const id = typeof source.id === 'string' && source.id.trim().length > 0
    ? source.id.trim()
    : '';

  if (!id) {
    throw new Error('History item is missing an id.');
  }

  const transcriptPath = requireSafeBackupFileName(source.transcriptPath, `History transcript path for ${id}`);
  const audioPath = typeof source.audioPath === 'string' ? source.audioPath : '';

  return {
    id,
    timestamp: typeof source.timestamp === 'number' && Number.isFinite(source.timestamp)
      ? Math.max(0, source.timestamp)
      : 0,
    duration: typeof source.duration === 'number' && Number.isFinite(source.duration)
      ? Math.max(0, source.duration)
      : 0,
    audioPath,
    transcriptPath,
    title: typeof source.title === 'string' ? source.title : '',
    previewText: typeof source.previewText === 'string' ? source.previewText : '',
    icon: typeof source.icon === 'string' ? source.icon : undefined,
    type: source.type === 'batch' ? 'batch' : 'recording',
    searchContent: typeof source.searchContent === 'string' ? source.searchContent : '',
    projectId: typeof source.projectId === 'string' && source.projectId.trim().length > 0
      ? source.projectId.trim()
      : null,
    status: 'complete',
  };
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

async function ensureAbsoluteDirectory(path: string): Promise<void> {
  await mkdir(path, { recursive: true });
}

async function ensureAppLocalDataDirectory(path: string): Promise<void> {
  await mkdir(path, { baseDir: BaseDirectory.AppLocalData, recursive: true });
}

async function cleanupAbsolutePath(path: string): Promise<void> {
  try {
    await remove(path, { recursive: true });
  } catch (error) {
    logger.warn?.('[Backup] Failed to clean temporary path:', path, error);
  }
}

async function readJsonFile<T>(path: string, label: string): Promise<T> {
  let content: string;
  try {
    content = await readTextFile(path);
  } catch (error) {
    throw new Error(`${label} could not be read: ${extractErrorMessage(error)}`);
  }

  try {
    return JSON.parse(content) as T;
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${extractErrorMessage(error)}`);
  }
}

async function readOptionalJsonFile<T>(path: string): Promise<T | null> {
  const found = await exists(path);
  if (!found) {
    return null;
  }

  return readJsonFile<T>(path, path);
}

async function writeAbsoluteJsonFile(path: string, value: unknown): Promise<void> {
  await writeTextFile(path, JSON.stringify(value, null, 2));
}

async function writeAppLocalDataJsonFile(path: string, value: unknown): Promise<void> {
  await writeTextFile(path, JSON.stringify(value, null, 2), {
    baseDir: BaseDirectory.AppLocalData,
  });
}

async function createTemporaryDirectory(prefix: string): Promise<string> {
  const systemTempDir = await tempDir();
  const dir = await join(
    systemTempDir,
    `sona-${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
  );
  await ensureAbsoluteDirectory(dir);
  return dir;
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

async function loadHistoryForBackup(): Promise<{
  items: HistoryItem[];
  transcriptFiles: Record<string, TranscriptSegment[]>;
  summaryFiles: Record<string, HistorySummaryPayload>;
}> {
  const items = (await historyService.getAll())
    .filter((item) => !isHistoryItemDraft(item))
    .map((item) => ({ ...item, status: 'complete' as const, draftSource: undefined }));
  const transcriptFiles: Record<string, TranscriptSegment[]> = {};
  const summaryFiles: Record<string, HistorySummaryPayload> = {};

  for (const item of items) {
    const transcriptPath = requireSafeBackupFileName(item.transcriptPath, `History transcript path for ${item.id}`);
    const segments = await historyService.loadTranscript(transcriptPath);
    if (!segments) {
      throw new Error(`History item "${item.title || item.id}" is missing its transcript file.`);
    }

    transcriptFiles[transcriptPath] = segments;

    const summary = await historyService.loadSummary(item.id);
    if (summary) {
      summaryFiles[item.id] = summary;
    }
  }

  return {
    items,
    transcriptFiles,
    summaryFiles,
  };
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

function buildManifest(args: {
  projectCount: number;
  historyItemCount: number;
  transcriptFileCount: number;
  summaryFileCount: number;
  automationRuleCount: number;
  automationProcessedEntryCount: number;
}): BackupManifestV1 {
  return {
    schemaVersion: BACKUP_SCHEMA_VERSION,
    createdAt: new Date().toISOString(),
    appVersion: packageJson.version,
    historyMode: BACKUP_HISTORY_MODE,
    scopes: {
      config: true,
      workspace: true,
      history: true,
      automation: true,
      analytics: true,
    },
    counts: {
      projects: args.projectCount,
      historyItems: args.historyItemCount,
      transcriptFiles: args.transcriptFileCount,
      summaryFiles: args.summaryFileCount,
      automationRules: args.automationRuleCount,
      automationProcessedEntries: args.automationProcessedEntryCount,
      analyticsFiles: 1,
    },
  };
}

async function stageExportBackup(stagingDir: string): Promise<BackupManifestV1> {
  const config = useConfigStore.getState().config;
  const [projects, history, automationRules, automationProcessedEntries, analyticsContent] = await Promise.all([
    loadProjectsForBackup(config),
    loadHistoryForBackup(),
    loadAutomationRules(),
    loadAutomationProcessedEntries(),
    loadAnalyticsContentForBackup(),
  ]);

  const manifest = buildManifest({
    projectCount: projects.length,
    historyItemCount: history.items.length,
    transcriptFileCount: Object.keys(history.transcriptFiles).length,
    summaryFileCount: Object.keys(history.summaryFiles).length,
    automationRuleCount: automationRules.length,
    automationProcessedEntryCount: automationProcessedEntries.length,
  });

  const configDir = await join(stagingDir, CONFIG_DIR_NAME);
  const projectsDir = await join(stagingDir, PROJECTS_DIR_NAME);
  const historyDir = await join(stagingDir, HISTORY_DIR_NAME);
  const automationDir = await join(stagingDir, AUTOMATION_DIR_NAME);
  const analyticsDir = await join(stagingDir, ANALYTICS_DIR_NAME);

  await Promise.all([
    ensureAbsoluteDirectory(configDir),
    ensureAbsoluteDirectory(projectsDir),
    ensureAbsoluteDirectory(historyDir),
    ensureAbsoluteDirectory(automationDir),
    ensureAbsoluteDirectory(analyticsDir),
  ]);

  await Promise.all([
    writeAbsoluteJsonFile(await join(stagingDir, 'manifest.json'), manifest),
    writeAbsoluteJsonFile(await join(configDir, CONFIG_FILE_NAME), config),
    writeAbsoluteJsonFile(await join(projectsDir, PROJECTS_INDEX_FILE), projects),
    writeAbsoluteJsonFile(await join(historyDir, PROJECTS_INDEX_FILE), history.items),
    writeAbsoluteJsonFile(await join(automationDir, AUTOMATION_RULES_FILE), automationRules),
    writeAbsoluteJsonFile(await join(automationDir, AUTOMATION_PROCESSED_FILE), automationProcessedEntries),
    writeTextFile(await join(analyticsDir, ANALYTICS_USAGE_FILE), analyticsContent),
  ]);

  await Promise.all(Object.entries(history.transcriptFiles).map(async ([fileName, segments]) => {
    await writeAbsoluteJsonFile(await join(historyDir, fileName), segments);
  }));

  await Promise.all(Object.entries(history.summaryFiles).map(async ([historyId, summary]) => {
    await writeAbsoluteJsonFile(await join(historyDir, `${historyId}${SUMMARY_FILE_SUFFIX}`), summary);
  }));

  return manifest;
}

async function writeLightHistoryImport(prepared: PreparedBackupImport): Promise<void> {
  await remove(APP_LOCAL_HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
  await ensureAppLocalDataDirectory(APP_LOCAL_HISTORY_DIR);

  await writeAppLocalDataJsonFile(`${APP_LOCAL_HISTORY_DIR}/${PROJECTS_INDEX_FILE}`, prepared.historyItems);

  await Promise.all(Object.entries(prepared.transcriptFiles).map(async ([fileName, segments]) => {
    await writeAppLocalDataJsonFile(`${APP_LOCAL_HISTORY_DIR}/${fileName}`, segments);
  }));

  await Promise.all(Object.entries(prepared.summaryFiles).map(async ([historyId, summary]) => {
    await writeAppLocalDataJsonFile(`${APP_LOCAL_HISTORY_DIR}/${historyId}${SUMMARY_FILE_SUFFIX}`, summary);
  }));
}

async function writeAnalyticsImport(analyticsContent: string): Promise<void> {
  await ensureAppLocalDataDirectory(APP_LOCAL_ANALYTICS_DIR);
  await writeTextFile(APP_LOCAL_ANALYTICS_USAGE_FILE, analyticsContent, {
    baseDir: BaseDirectory.AppLocalData,
  });
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

async function loadPreparedImportFromExtractionDir(archivePath: string, extractionDir: string): Promise<PreparedBackupImport> {
  const manifest = validateManifest(await readJsonFile<unknown>(
    await join(extractionDir, 'manifest.json'),
    'Backup manifest',
  ));

  const config = ensureRecord(await readJsonFile<unknown>(
    await join(extractionDir, CONFIG_DIR_NAME, CONFIG_FILE_NAME),
    'Backup config',
  ), 'Backup config') as unknown as AppConfig;

  const projectsRaw = ensureArray(await readJsonFile<unknown>(
    await join(extractionDir, PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE),
    'Backup projects',
  ), 'Backup projects');
  const projects = projectsRaw.map((item) => normalizeProjectRecord(item as Partial<ProjectRecord>));

  const historyItemsRaw = ensureArray(await readJsonFile<unknown>(
    await join(extractionDir, HISTORY_DIR_NAME, PROJECTS_INDEX_FILE),
    'Backup history index',
  ), 'Backup history index');
  const historyItems = historyItemsRaw.map((item) => normalizeHistoryItemForImport(item));
  const transcriptFiles: Record<string, TranscriptSegment[]> = {};
  const summaryFiles: Record<string, HistorySummaryPayload> = {};

  for (const item of historyItems) {
    if (item.status === 'draft') {
      throw new Error(`Backup history item "${item.id}" is a draft and cannot be imported.`);
    }

    const transcriptPath = item.transcriptPath;
    transcriptFiles[transcriptPath] = ensureArray(await readJsonFile<unknown>(
      await join(extractionDir, HISTORY_DIR_NAME, transcriptPath),
      `Transcript for history item ${item.id}`,
    ), `Transcript for history item ${item.id}`) as TranscriptSegment[];

    const summaryPath = await join(extractionDir, HISTORY_DIR_NAME, `${item.id}${SUMMARY_FILE_SUFFIX}`);
    const summary = await readOptionalJsonFile<unknown>(summaryPath);
    if (summary) {
      summaryFiles[item.id] = ensureRecord(summary, `Summary for history item ${item.id}`) as unknown as HistorySummaryPayload;
    }
  }

  const automationRulesRaw = ensureArray(await readJsonFile<unknown>(
    await join(extractionDir, AUTOMATION_DIR_NAME, AUTOMATION_RULES_FILE),
    'Backup automation rules',
  ), 'Backup automation rules');
  const automationProcessedEntriesRaw = ensureArray(await readJsonFile<unknown>(
    await join(extractionDir, AUTOMATION_DIR_NAME, AUTOMATION_PROCESSED_FILE),
    'Backup automation processed entries',
  ), 'Backup automation processed entries');

  const automationRules = automationRulesRaw.map((item) => normalizeAutomationRule(item));
  const automationProcessedEntries = automationProcessedEntriesRaw.map((item) => normalizeAutomationProcessedEntry(item));
  const analyticsPath = await join(extractionDir, ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE);
  const analyticsContent = await readTextFile(analyticsPath);
  ensureRecord(JSON.parse(analyticsContent), 'Backup analytics');

  if (manifest.counts.projects !== projects.length) {
    throw new Error('Backup project count does not match the manifest.');
  }

  if (manifest.counts.historyItems !== historyItems.length) {
    throw new Error('Backup history count does not match the manifest.');
  }

  if (manifest.counts.transcriptFiles !== Object.keys(transcriptFiles).length) {
    throw new Error('Backup transcript count does not match the manifest.');
  }

  if (manifest.counts.summaryFiles !== Object.keys(summaryFiles).length) {
    throw new Error('Backup summary count does not match the manifest.');
  }

  if (manifest.counts.automationRules !== automationRules.length) {
    throw new Error('Backup automation-rule count does not match the manifest.');
  }

  if (manifest.counts.automationProcessedEntries !== automationProcessedEntries.length) {
    throw new Error('Backup processed-entry count does not match the manifest.');
  }

  if (manifest.counts.analyticsFiles !== 1) {
    throw new Error('Backup analytics count does not match the manifest.');
  }

  return {
    archivePath,
    extractionDir,
    manifest,
    config,
    projects,
    historyItems,
    transcriptFiles,
    summaryFiles,
    automationRules,
    automationProcessedEntries,
    analyticsContent,
  };
}

export async function exportBackup(options?: {
  archivePath?: string;
}): Promise<ExportBackupResult | null> {
  ensureBackupOperationsIdle();

  const archivePath = options?.archivePath || await pickExportArchivePath();
  if (!archivePath) {
    return null;
  }

  const stagingDir = await createTemporaryDirectory('backup-export');

  try {
    const manifest = await stageExportBackup(stagingDir);
    await invoke('create_tar_bz2', {
      sourceDir: stagingDir,
      archivePath,
    });

    return {
      archivePath,
      manifest,
    };
  } finally {
    await cleanupAbsolutePath(stagingDir);
  }
}

export async function prepareImportBackup(options?: {
  archivePath?: string;
}): Promise<PreparedBackupImport | null> {
  ensureBackupOperationsIdle();

  const archivePath = options?.archivePath || await pickImportArchivePath();
  if (!archivePath) {
    return null;
  }

  const extractionDir = await createTemporaryDirectory('backup-import');

  try {
    await invoke('extract_tar_bz2', {
      archivePath,
      targetDir: extractionDir,
    });

    return await loadPreparedImportFromExtractionDir(archivePath, extractionDir);
  } catch (error) {
    await cleanupAbsolutePath(extractionDir);
    throw error;
  }
}

export async function disposePreparedImport(prepared: PreparedBackupImport): Promise<void> {
  await cleanupAbsolutePath(prepared.extractionDir);
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
    await writeLightHistoryImport(prepared);
    await saveAutomationRules(prepared.automationRules);
    await saveAutomationProcessedEntries(prepared.automationProcessedEntries);
    await writeAnalyticsImport(prepared.analyticsContent);

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

    await cleanupAbsolutePath(prepared.extractionDir);
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
