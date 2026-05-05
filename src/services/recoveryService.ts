import { convertFileSrc } from '@tauri-apps/api/core';
import type { BatchQueueItem } from '../types/batchQueue';
import type { RecoverySnapshot, RecoveredQueueItem } from '../types/recovery';
import { logger } from '../utils/logger';
import {
    recoveryLoadSnapshot,
    recoveryPersistQueueSnapshot,
    recoverySaveSnapshot,
} from './tauri/recovery';

const RECOVERY_VERSION = 1;
const RECOVERY_WRITE_DEBOUNCE_MS = 120;

const pendingAutomationRecoveryGuard = new Set<string>();
const activeAutomationRecoveryGuard = new Set<string>();

let pendingQueueSnapshotWrite: BatchQueueItem[] | null = null;
const pendingResolvedRecoveryIds = new Set<string>();
let snapshotWriteTimer: ReturnType<typeof setTimeout> | null = null;
let snapshotWriteChain = Promise.resolve();

function buildAutomationRecoveryGuardKey(ruleId: string, sourceFingerprint: string): string {
    return `${ruleId}::${sourceFingerprint}`;
}

function emptySnapshot(): RecoverySnapshot {
    return {
        version: RECOVERY_VERSION,
        updatedAt: null,
        items: [],
    };
}

function cloneQueueItemsForRecovery(queueItems: BatchQueueItem[]): BatchQueueItem[] {
    return queueItems.map((item) => {
        const clonedItem: BatchQueueItem = {
            ...item,
            segments: Array.isArray(item.segments)
                ? item.segments.map((segment) => ({ ...segment }))
                : [],
        };
        if (item.fileStat) {
            clonedItem.fileStat = { ...item.fileStat };
        }
        return clonedItem;
    });
}

async function flushPendingSnapshotWrite(): Promise<void> {
    if (snapshotWriteTimer) {
        clearTimeout(snapshotWriteTimer);
        snapshotWriteTimer = null;
    }

    const queueItems = pendingQueueSnapshotWrite;
    const resolvedIds = Array.from(pendingResolvedRecoveryIds);
    pendingQueueSnapshotWrite = null;
    pendingResolvedRecoveryIds.clear();

    if (!queueItems) {
        return;
    }

    snapshotWriteChain = snapshotWriteChain
        .catch(() => undefined)
        .then(() => recoveryPersistQueueSnapshot(queueItems, resolvedIds));

    await snapshotWriteChain;

    if (pendingQueueSnapshotWrite) {
        await flushPendingSnapshotWrite();
    }
}

function queueSnapshotWrite(
    queueItems: BatchQueueItem[],
    immediate = false,
    resolvedIds: string[] = [],
): void {
    pendingQueueSnapshotWrite = cloneQueueItemsForRecovery(queueItems);
    resolvedIds
        .map((id) => id.trim())
        .filter(Boolean)
        .forEach((id) => pendingResolvedRecoveryIds.add(id));

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

export function toBatchQueueItem(item: RecoveredQueueItem): BatchQueueItem {
    return {
        id: item.id,
        recoveryId: item.id,
        filename: item.filename,
        filePath: item.filePath,
        status: 'pending',
        progress: 0,
        segments: Array.isArray(item.segments) ? item.segments : [],
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
        const snapshot = await recoveryLoadSnapshot();
        syncPendingAutomationRecoveryGuard(snapshot.items);
        return snapshot;
    } catch (error) {
        logger.error('[Recovery] Failed to load recovery snapshot:', error);
        syncPendingAutomationRecoveryGuard([]);
        return emptySnapshot();
    }
}

export async function saveRecoveredItems(items: RecoveredQueueItem[]): Promise<void> {
    await flushPendingSnapshotWrite();
    const snapshot = await recoverySaveSnapshot(items);
    syncPendingAutomationRecoveryGuard(snapshot.items);
}

export function persistQueueRecoverySnapshot(
    queueItems: BatchQueueItem[],
    options?: { immediate?: boolean; resolvedIds?: string[] },
): void {
    queueSnapshotWrite(queueItems, Boolean(options?.immediate), options?.resolvedIds ?? []);
}

export async function flushRecoverySnapshotWrites(): Promise<void> {
    await flushPendingSnapshotWrite();
    await snapshotWriteChain.catch(() => undefined);
}

export function resetRecoveryRuntimeForTests(): void {
    pendingAutomationRecoveryGuard.clear();
    activeAutomationRecoveryGuard.clear();
    pendingQueueSnapshotWrite = null;
    pendingResolvedRecoveryIds.clear();
    if (snapshotWriteTimer) {
        clearTimeout(snapshotWriteTimer);
        snapshotWriteTimer = null;
    }
    snapshotWriteChain = Promise.resolve();
}
