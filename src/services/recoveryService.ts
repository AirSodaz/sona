import { convertFileSrc } from '@tauri-apps/api/core';
import {
    BaseDirectory,
    exists,
    mkdir,
    readTextFile,
    writeTextFile,
} from '@tauri-apps/plugin-fs';
import { getPathStatusMap, isRuntimePathFile } from './pathStatusService';
import type { BatchQueueItem } from '../types/batchQueue';
import type { RecoverySnapshot, RecoveredQueueItem } from '../types/recovery';
import { logger } from '../utils/logger';
import { normalizeTranscriptSegments } from '../utils/transcriptTiming';

const RECOVERY_VERSION = 1;
const RECOVERY_DIR = 'recovery';
const RECOVERY_FILE = `${RECOVERY_DIR}/queue-recovery.json`;
const RECOVERY_WRITE_DEBOUNCE_MS = 120;

const pendingAutomationRecoveryGuard = new Set<string>();
const activeAutomationRecoveryGuard = new Set<string>();

let pendingSnapshotWrite: RecoverySnapshot | null = null;
let snapshotWriteTimer: ReturnType<typeof setTimeout> | null = null;
let snapshotWriteChain = Promise.resolve();

function buildAutomationRecoveryGuardKey(ruleId: string, sourceFingerprint: string): string {
    return `${ruleId}::${sourceFingerprint}`;
}

function toRecoverySource(item: Pick<BatchQueueItem, 'origin'>): RecoveredQueueItem['source'] {
    return item.origin === 'automation' ? 'automation' : 'batch_import';
}

function createSnapshot(items: RecoveredQueueItem[]): RecoverySnapshot {
    return {
        version: RECOVERY_VERSION,
        updatedAt: items.length > 0 ? Date.now() : null,
        items,
    };
}

function cloneRecoveredItem(item: RecoveredQueueItem): RecoveredQueueItem {
    return {
        ...item,
        segments: normalizeTranscriptSegments(Array.isArray(item.segments) ? item.segments : []),
        fileStat: item.fileStat ? { ...item.fileStat } : undefined,
        exportConfig: item.exportConfig ? { ...item.exportConfig } : null,
        stageConfig: item.stageConfig ? { ...item.stageConfig } : null,
        resolvedConfigSnapshot: item.resolvedConfigSnapshot
            ? { ...item.resolvedConfigSnapshot }
            : undefined,
    };
}

async function ensureRecoveryStorage(): Promise<void> {
    const recoveryDirExists = await exists(RECOVERY_DIR, { baseDir: BaseDirectory.AppLocalData });
    if (!recoveryDirExists) {
        await mkdir(RECOVERY_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
    }

    const recoveryFileExists = await exists(RECOVERY_FILE, { baseDir: BaseDirectory.AppLocalData });
    if (!recoveryFileExists) {
        await writeTextFile(RECOVERY_FILE, JSON.stringify(createSnapshot([]), null, 2), {
            baseDir: BaseDirectory.AppLocalData,
        });
    }
}

async function writeRecoverySnapshot(snapshot: RecoverySnapshot): Promise<void> {
    await ensureRecoveryStorage();
    await writeTextFile(RECOVERY_FILE, JSON.stringify(snapshot, null, 2), {
        baseDir: BaseDirectory.AppLocalData,
    });
}

async function flushPendingSnapshotWrite(): Promise<void> {
    if (snapshotWriteTimer) {
        clearTimeout(snapshotWriteTimer);
        snapshotWriteTimer = null;
    }

    const snapshot = pendingSnapshotWrite;
    pendingSnapshotWrite = null;

    if (!snapshot) {
        return;
    }

    snapshotWriteChain = snapshotWriteChain
        .catch(() => undefined)
        .then(() => writeRecoverySnapshot(snapshot));

    await snapshotWriteChain;

    if (pendingSnapshotWrite) {
        await flushPendingSnapshotWrite();
    }
}

function queueSnapshotWrite(snapshot: RecoverySnapshot, immediate = false): void {
    pendingSnapshotWrite = snapshot;

    if (immediate) {
        void flushPendingSnapshotWrite();
        return;
    }

    if (snapshotWriteTimer) {
        return;
    }

    snapshotWriteTimer = setTimeout(() => {
        snapshotWriteTimer = null;
        void flushPendingSnapshotWrite();
    }, RECOVERY_WRITE_DEBOUNCE_MS);
}

function canTrackQueueItemForRecovery(item: BatchQueueItem): boolean {
    return item.status === 'pending' || item.status === 'processing';
}

export function toRecoveredQueueItem(item: BatchQueueItem): RecoveredQueueItem {
    return {
        id: item.recoveryId || item.id,
        filename: item.filename,
        filePath: item.filePath,
        source: toRecoverySource(item),
        resolution: 'pending',
        progress: item.progress,
        segments: normalizeTranscriptSegments(item.segments),
        projectId: item.projectId,
        historyId: item.historyId,
        historyTitle: item.historyTitle,
        lastKnownStage: item.lastKnownStage || 'queued',
        updatedAt: Date.now(),
        hasSourceFile: true,
        canResume: true,
        automationRuleId: item.automationRuleId,
        automationRuleName: item.automationRuleName,
        resolvedConfigSnapshot: item.resolvedConfigSnapshot
            ? { ...item.resolvedConfigSnapshot }
            : undefined,
        exportConfig: item.exportConfig ? { ...item.exportConfig } : null,
        stageConfig: item.stageConfig ? { ...item.stageConfig } : null,
        sourceFingerprint: item.sourceFingerprint,
        fileStat: item.fileStat ? { ...item.fileStat } : undefined,
        exportFileNamePrefix: item.exportFileNamePrefix,
    };
}

export function toRecoveredQueueItems(queueItems: BatchQueueItem[]): RecoveredQueueItem[] {
    return queueItems
        .filter(canTrackQueueItemForRecovery)
        .map((item) => toRecoveredQueueItem(item));
}

export function toBatchQueueItem(item: RecoveredQueueItem): BatchQueueItem {
    return {
        id: item.id,
        recoveryId: item.id,
        filename: item.filename,
        filePath: item.filePath,
        status: 'pending',
        progress: 0,
        segments: normalizeTranscriptSegments(item.segments),
        audioUrl: convertFileSrc(item.filePath),
        historyId: item.historyId,
        historyTitle: item.historyTitle,
        projectId: item.projectId,
        origin: item.source === 'automation' ? 'automation' : 'manual',
        automationRuleId: item.automationRuleId,
        automationRuleName: item.automationRuleName,
        resolvedConfigSnapshot: item.resolvedConfigSnapshot
            ? { ...item.resolvedConfigSnapshot }
            : undefined,
        exportConfig: item.exportConfig ? { ...item.exportConfig } : null,
        stageConfig: item.stageConfig ? { ...item.stageConfig } : null,
        sourceFingerprint: item.sourceFingerprint,
        fileStat: item.fileStat ? { ...item.fileStat } : undefined,
        exportFileNamePrefix: item.exportFileNamePrefix,
        lastKnownStage: 'queued',
    };
}

export function syncPendingAutomationRecoveryGuard(items: RecoveredQueueItem[]): void {
    pendingAutomationRecoveryGuard.clear();

    items.forEach((item) => {
        if (
            item.source === 'automation'
            && item.automationRuleId
            && item.sourceFingerprint
            && item.resolution === 'pending'
        ) {
            pendingAutomationRecoveryGuard.add(
                buildAutomationRecoveryGuardKey(item.automationRuleId, item.sourceFingerprint),
            );
        }
    });
}

export function markAutomationRecoveryItemsResumed(items: RecoveredQueueItem[]): void {
    items.forEach((item) => {
        if (!item.automationRuleId || !item.sourceFingerprint) {
            return;
        }

        const key = buildAutomationRecoveryGuardKey(item.automationRuleId, item.sourceFingerprint);
        pendingAutomationRecoveryGuard.delete(key);
        activeAutomationRecoveryGuard.add(key);
    });
}

export function clearAutomationRecoveryGuardEntry(ruleId: string, sourceFingerprint: string): void {
    const key = buildAutomationRecoveryGuardKey(ruleId, sourceFingerprint);
    pendingAutomationRecoveryGuard.delete(key);
    activeAutomationRecoveryGuard.delete(key);
}

export function isAutomationRecoveryBlocked(ruleId: string, sourceFingerprint: string): boolean {
    const key = buildAutomationRecoveryGuardKey(ruleId, sourceFingerprint);
    return pendingAutomationRecoveryGuard.has(key) || activeAutomationRecoveryGuard.has(key);
}

export async function loadRecoverySnapshot(): Promise<RecoverySnapshot> {
    try {
        await ensureRecoveryStorage();
        const content = await readTextFile(RECOVERY_FILE, { baseDir: BaseDirectory.AppLocalData });
        const parsed = JSON.parse(content) as Partial<RecoverySnapshot> | null;
        const rawItems = Array.isArray(parsed?.items) ? parsed.items : [];
        const pathStatusMap = await getPathStatusMap(
            rawItems.map((rawItem) => (rawItem as RecoveredQueueItem).filePath),
        );

        const items = rawItems.map((rawItem) => {
            const item = rawItem as RecoveredQueueItem;
            const resolution = item.resolution || 'pending';
            const defaultHasSourceFile = typeof item.hasSourceFile === 'boolean' ? item.hasSourceFile : true;
            const defaultCanResume = typeof item.canResume === 'boolean'
                ? item.canResume
                : defaultHasSourceFile && resolution === 'pending';
            const pathStatus = pathStatusMap[item.filePath];

            const hasSourceFile = isRuntimePathFile(pathStatus)
                ? true
                : pathStatus?.kind === 'missing' || pathStatus?.kind === 'directory'
                    ? false
                    : defaultHasSourceFile;
            const canResume = isRuntimePathFile(pathStatus)
                ? resolution === 'pending'
                : pathStatus?.kind === 'missing' || pathStatus?.kind === 'directory'
                    ? false
                    : defaultCanResume;

            return cloneRecoveredItem({
                ...item,
                source: item.source || (item.automationRuleId ? 'automation' : 'batch_import'),
                resolution,
                lastKnownStage: item.lastKnownStage || 'queued',
                updatedAt: typeof item.updatedAt === 'number' ? item.updatedAt : Date.now(),
                hasSourceFile,
                canResume,
            });
        });

        const snapshot = {
            version: parsed?.version || RECOVERY_VERSION,
            updatedAt: typeof parsed?.updatedAt === 'number' ? parsed.updatedAt : null,
            items,
        } satisfies RecoverySnapshot;

        syncPendingAutomationRecoveryGuard(snapshot.items);
        return snapshot;
    } catch (error) {
        logger.error('[Recovery] Failed to load recovery snapshot:', error);
        syncPendingAutomationRecoveryGuard([]);
        return createSnapshot([]);
    }
}

export async function saveRecoveredItems(items: RecoveredQueueItem[]): Promise<void> {
    const pendingItems = items
        .filter((item) => item.resolution === 'pending')
        .map((item) => cloneRecoveredItem(item));
    syncPendingAutomationRecoveryGuard(pendingItems);
    await writeRecoverySnapshot(createSnapshot(pendingItems));
}

export function persistQueueRecoverySnapshot(
    queueItems: BatchQueueItem[],
    options?: { immediate?: boolean },
): void {
    const snapshot = createSnapshot(toRecoveredQueueItems(queueItems));
    queueSnapshotWrite(snapshot, Boolean(options?.immediate));
}

export async function flushRecoverySnapshotWrites(): Promise<void> {
    await flushPendingSnapshotWrite();
    await snapshotWriteChain.catch(() => undefined);
}

export function resetRecoveryRuntimeForTests(): void {
    pendingAutomationRecoveryGuard.clear();
    activeAutomationRecoveryGuard.clear();
    pendingSnapshotWrite = null;
    if (snapshotWriteTimer) {
        clearTimeout(snapshotWriteTimer);
        snapshotWriteTimer = null;
    }
    snapshotWriteChain = Promise.resolve();
}
