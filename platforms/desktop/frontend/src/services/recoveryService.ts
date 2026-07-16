import type { BatchQueueItem } from '../types/batchQueue';
import type { RecoverySnapshot, RecoveredQueueItem } from '../types/recovery';
import type {
    RecoveredQueueItem_Serialize as CoreRecoveredQueueItem,
    RecoveredTranscriptSegment_Serialize,
    RecoveryItemInput_Serialize,
    RecoverySnapshot_Serialize as CoreRecoverySnapshot,
} from '../bindings';
import type { AppConfig } from '../types/config';
import type { AutomationExportConfig, AutomationStageConfig } from '../types/automation';
import type { TranscriptSegment } from '../types/transcript';
import { normalizeSpeakerAttribution, normalizeSpeakerTag } from '../types/speaker';
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

function isRecord(value: unknown): value is Record<string, unknown> {
    return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function normalizeRecoveredSegment(
    segment: RecoveredTranscriptSegment_Serialize,
): TranscriptSegment {
    const speaker = normalizeSpeakerTag(segment.speaker);
    const speakerAttribution = normalizeSpeakerAttribution(segment.speakerAttribution);

    return {
        id: segment.id,
        text: segment.text,
        start: segment.start,
        end: segment.end,
        isFinal: segment.isFinal,
        ...(segment.timing
            ? {
                timing: {
                    ...segment.timing,
                    units: segment.timing.units.map((unit) => ({
                        text: unit.text ?? '',
                        start: unit.start ?? segment.start,
                        end: unit.end ?? segment.end,
                    })),
                },
            }
            : {}),
        ...(segment.tokens ? { tokens: [...segment.tokens] } : {}),
        ...(segment.timestamps ? { timestamps: [...segment.timestamps] } : {}),
        ...(segment.durations ? { durations: [...segment.durations] } : {}),
        ...(segment.translation != null ? { translation: segment.translation } : {}),
        ...(speaker != null ? { speaker } : {}),
        ...(speakerAttribution != null ? { speakerAttribution } : {}),
    };
}

function normalizeRecoveredItem(item: CoreRecoveredQueueItem): RecoveredQueueItem {
    return {
        id: item.id,
        filename: item.filename,
        filePath: item.filePath,
        source: item.source,
        resolution: item.resolution,
        progress: item.progress,
        segments: item.segments.map(normalizeRecoveredSegment),
        projectId: item.projectId,
        ...(item.historyId != null ? { historyId: item.historyId } : {}),
        ...(item.historyTitle != null ? { historyTitle: item.historyTitle } : {}),
        lastKnownStage: item.lastKnownStage,
        updatedAt: item.updatedAt,
        hasSourceFile: item.hasSourceFile,
        canResume: item.canResume,
        ...(item.automationRuleId != null ? { automationRuleId: item.automationRuleId } : {}),
        ...(item.automationRuleName != null ? { automationRuleName: item.automationRuleName } : {}),
        ...(isRecord(item.resolvedConfigSnapshot)
            ? {
                resolvedConfigSnapshot: {
                    ...item.resolvedConfigSnapshot,
                } as unknown as AppConfig,
            }
            : {}),
        exportConfig: isRecord(item.exportConfig)
            ? { ...item.exportConfig } as unknown as AutomationExportConfig
            : null,
        stageConfig: isRecord(item.stageConfig)
            ? { ...item.stageConfig } as unknown as AutomationStageConfig
            : null,
        ...(item.sourceFingerprint != null
            ? { sourceFingerprint: item.sourceFingerprint }
            : {}),
        ...(item.fileStat != null ? { fileStat: { ...item.fileStat } } : {}),
        ...(item.exportFileNamePrefix != null
            ? { exportFileNamePrefix: item.exportFileNamePrefix }
            : {}),
    };
}

function normalizeRecoverySnapshot(snapshot: CoreRecoverySnapshot): RecoverySnapshot {
    return {
        version: snapshot.version,
        updatedAt: snapshot.updatedAt,
        items: snapshot.items.map(normalizeRecoveredItem),
    };
}

function recoveredItemInput(item: RecoveredQueueItem): RecoveryItemInput_Serialize {
    return {
        id: item.id,
        filename: item.filename,
        filePath: item.filePath,
        source: item.source,
        resolution: item.resolution,
        progress: item.progress,
        segments: item.segments.map((segment) => ({ ...segment })),
        projectId: item.projectId,
        ...(item.historyId != null ? { historyId: item.historyId } : {}),
        ...(item.historyTitle != null ? { historyTitle: item.historyTitle } : {}),
        lastKnownStage: item.lastKnownStage,
        updatedAt: item.updatedAt,
        hasSourceFile: item.hasSourceFile,
        canResume: item.canResume,
        ...(item.automationRuleId != null ? { automationRuleId: item.automationRuleId } : {}),
        ...(item.automationRuleName != null ? { automationRuleName: item.automationRuleName } : {}),
        ...(item.resolvedConfigSnapshot != null
            ? { resolvedConfigSnapshot: item.resolvedConfigSnapshot }
            : {}),
        exportConfig: item.exportConfig ?? null,
        stageConfig: item.stageConfig ?? null,
        ...(item.sourceFingerprint != null
            ? { sourceFingerprint: item.sourceFingerprint }
            : {}),
        ...(item.fileStat != null ? { fileStat: { ...item.fileStat } } : {}),
        ...(item.exportFileNamePrefix != null
            ? { exportFileNamePrefix: item.exportFileNamePrefix }
            : {}),
    };
}

function queueItemInput(item: BatchQueueItem): RecoveryItemInput_Serialize {
    return {
        id: item.id,
        ...(item.recoveryId != null ? { recoveryId: item.recoveryId } : {}),
        filename: item.filename,
        filePath: item.filePath,
        ...(item.origin != null ? { origin: item.origin } : {}),
        status: item.status,
        progress: item.progress,
        segments: item.segments.map((segment) => ({ ...segment })),
        projectId: item.projectId,
        ...(item.historyId != null ? { historyId: item.historyId } : {}),
        ...(item.historyTitle != null ? { historyTitle: item.historyTitle } : {}),
        ...(item.lastKnownStage != null ? { lastKnownStage: item.lastKnownStage } : {}),
        ...(item.automationRuleId != null ? { automationRuleId: item.automationRuleId } : {}),
        ...(item.automationRuleName != null ? { automationRuleName: item.automationRuleName } : {}),
        ...(item.resolvedConfigSnapshot != null
            ? { resolvedConfigSnapshot: item.resolvedConfigSnapshot }
            : {}),
        ...(item.exportConfig !== undefined ? { exportConfig: item.exportConfig } : {}),
        ...(item.stageConfig !== undefined ? { stageConfig: item.stageConfig } : {}),
        ...(item.sourceFingerprint != null
            ? { sourceFingerprint: item.sourceFingerprint }
            : {}),
        ...(item.fileStat != null ? { fileStat: { ...item.fileStat } } : {}),
        ...(item.exportFileNamePrefix != null
            ? { exportFileNamePrefix: item.exportFileNamePrefix }
            : {}),
    };
}

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
        .then(() => recoveryPersistQueueSnapshot(queueItems.map(queueItemInput), resolvedIds));

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
        audioUrl: null,
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
        const snapshot = normalizeRecoverySnapshot(await recoveryLoadSnapshot());
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
    const snapshot = normalizeRecoverySnapshot(
        await recoverySaveSnapshot(items.map(recoveredItemInput)),
    );
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
