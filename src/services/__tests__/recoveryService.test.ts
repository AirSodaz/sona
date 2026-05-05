import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BatchQueueItem } from '../../types/batchQueue';
import type { RecoveredQueueItem } from '../../types/recovery';
import { TauriCommand } from '../tauri/commands';

const testContext = vi.hoisted(() => ({
    invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
    invoke: (...args: unknown[]) => testContext.invokeMock(...args),
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        error: vi.fn(),
    },
}));

import {
    clearAutomationRecoveryGuardEntry,
    flushRecoverySnapshotWrites,
    isAutomationRecoveryBlocked,
    loadRecoverySnapshot,
    markAutomationRecoveryItemsResumed,
    persistQueueRecoverySnapshot,
    resetRecoveryRuntimeForTests,
    saveRecoveredItems,
    toBatchQueueItem,
} from '../recoveryService';

function recoveryItem(overrides: Partial<RecoveredQueueItem> = {}): RecoveredQueueItem {
    return {
        id: 'recovery-1',
        filename: 'meeting.wav',
        filePath: 'C:\\watch\\meeting.wav',
        source: 'batch_import',
        resolution: 'pending',
        progress: 30,
        segments: [],
        projectId: null,
        lastKnownStage: 'transcribing',
        updatedAt: 100,
        hasSourceFile: true,
        canResume: true,
        exportConfig: null,
        stageConfig: null,
        ...overrides,
    };
}

function queueItem(overrides: Partial<BatchQueueItem> = {}): BatchQueueItem {
    return {
        id: 'queue-1',
        filename: 'meeting.wav',
        filePath: 'C:\\watch\\meeting.wav',
        status: 'pending',
        progress: 0,
        segments: [],
        projectId: null,
        lastKnownStage: 'queued',
        ...overrides,
    };
}

describe('recoveryService', () => {
    beforeEach(() => {
        resetRecoveryRuntimeForTests();
        vi.clearAllMocks();
        testContext.invokeMock.mockImplementation(async (command: string) => {
            if (command === TauriCommand.recovery.loadSnapshot) {
                return { version: 1, updatedAt: null, items: [] };
            }
            if (command === TauriCommand.recovery.saveSnapshot) {
                return { version: 1, updatedAt: null, items: [] };
            }
            if (command === TauriCommand.recovery.persistQueueSnapshot) {
                return undefined;
            }
            throw new Error(`Unexpected command: ${command}`);
        });
    });

    it('delegates queue persistence to the Rust recovery repository', async () => {
        const queueItems: BatchQueueItem[] = [
            queueItem({ id: 'pending-1', status: 'pending' }),
            queueItem({ id: 'complete-1', status: 'complete', progress: 100 }),
        ];

        persistQueueRecoverySnapshot(queueItems, { immediate: true });
        await flushRecoverySnapshotWrites();

        expect(testContext.invokeMock).toHaveBeenCalledWith(
            TauriCommand.recovery.persistQueueSnapshot,
            { queueItems },
        );
    });

    it('passes resolved recovery ids when persisting queue snapshots', async () => {
        const queueItems = [queueItem({ id: 'pending-1', recoveryId: 'recovery-1' })];

        persistQueueRecoverySnapshot(queueItems, {
            immediate: true,
            resolvedIds: ['recovery-cleared'],
        });
        await flushRecoverySnapshotWrites();

        expect(testContext.invokeMock).toHaveBeenCalledWith(
            TauriCommand.recovery.persistQueueSnapshot,
            {
                queueItems,
                resolvedIds: ['recovery-cleared'],
            },
        );
    });

    it('flushes pending queue snapshot writes before saving recovered items', async () => {
        const queueItems = [queueItem({ id: 'pending-before-save' })];

        persistQueueRecoverySnapshot(queueItems);
        await saveRecoveredItems([]);

        expect(testContext.invokeMock).toHaveBeenNthCalledWith(
            1,
            TauriCommand.recovery.persistQueueSnapshot,
            { queueItems },
        );
        expect(testContext.invokeMock).toHaveBeenNthCalledWith(
            2,
            TauriCommand.recovery.saveSnapshot,
            { items: [] },
        );
    });

    it('loads snapshots through Rust and keeps automation items blocked', async () => {
        const automationItem = recoveryItem({
            id: 'recovery-automation-1',
            source: 'automation',
            automationRuleId: 'rule-1',
            sourceFingerprint: 'fp-automation-1',
        });
        testContext.invokeMock.mockResolvedValueOnce({
            version: 1,
            updatedAt: 200,
            items: [automationItem],
        });

        const snapshot = await loadRecoverySnapshot();

        expect(snapshot.items).toEqual([automationItem]);
        expect(testContext.invokeMock).toHaveBeenCalledWith(TauriCommand.recovery.loadSnapshot);
        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(true);

        markAutomationRecoveryItemsResumed(snapshot.items);
        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(true);

        clearAutomationRecoveryGuardEntry('rule-1', 'fp-automation-1');
        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(false);
    });

    it('passes recovery items to Rust when saving so Rust owns filtering and normalization', async () => {
        const pendingItem = recoveryItem({ id: 'pending-1' });
        const discardedItem = recoveryItem({
            id: 'discarded-1',
            resolution: 'discarded',
            canResume: false,
        });
        testContext.invokeMock.mockResolvedValueOnce({
            version: 1,
            updatedAt: 300,
            items: [pendingItem],
        });

        await saveRecoveredItems([pendingItem, discardedItem]);

        expect(testContext.invokeMock).toHaveBeenCalledWith(
            TauriCommand.recovery.saveSnapshot,
            { items: [pendingItem, discardedItem] },
        );
    });

    it('adapts recovered queue items back into batch queue items for resume', () => {
        const segment = {
            id: 'segment-1',
            text: 'Hello',
            start: 0,
            end: 1,
            isFinal: true,
            timing: {
                level: 'segment' as const,
                source: 'derived' as const,
                units: [{ text: 'Hello', start: 0, end: 1 }],
            },
        };
        const item = recoveryItem({
            id: 'recovery-automation-1',
            source: 'automation',
            segments: [segment],
            automationRuleId: 'rule-1',
            automationRuleName: 'Inbox',
            resolvedConfigSnapshot: {} as never,
            exportConfig: null,
            stageConfig: null,
            sourceFingerprint: 'fp-1',
            fileStat: { size: 42, mtimeMs: 1000 },
            exportFileNamePrefix: 'meeting',
        });

        const queue = toBatchQueueItem(item);

        expect(queue).toEqual(expect.objectContaining({
            id: 'recovery-automation-1',
            recoveryId: 'recovery-automation-1',
            status: 'pending',
            progress: 0,
            audioUrl: 'asset://C:\\watch\\meeting.wav',
            origin: 'automation',
            lastKnownStage: 'queued',
            automationRuleId: 'rule-1',
            sourceFingerprint: 'fp-1',
            fileStat: { size: 42, mtimeMs: 1000 },
            exportFileNamePrefix: 'meeting',
        }));
        expect(queue.segments).toEqual([segment]);
    });
});
