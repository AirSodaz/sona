import { beforeEach, describe, expect, it, vi } from 'vitest';

const testContext = vi.hoisted(() => ({
    loadRecoverySnapshotMock: vi.fn(),
    markAutomationRecoveryItemsResumedMock: vi.fn(),
    saveRecoveredItemsMock: vi.fn(),
    enqueueRecoveredItemsMock: vi.fn(),
    markRecoveryItemDiscardedMock: vi.fn(),
    upsertTaskLedgerRecordMock: vi.fn(),
    patchTaskLedgerRecordMock: vi.fn(),
    buildRecoveryTaskLedgerRecordMock: vi.fn((item: any) => ({
        id: `recovery-${item.id}`,
        kind: 'recovery',
        status: 'recoverable',
        title: item.filename,
        progress: item.progress,
        createdAt: item.updatedAt,
        updatedAt: item.updatedAt,
        retryable: item.canResume,
        cancelable: false,
        recoverable: item.canResume,
    })),
}));

vi.mock('../../services/recoveryService', () => ({
    loadRecoverySnapshot: (...args: unknown[]) => testContext.loadRecoverySnapshotMock(...args),
    markAutomationRecoveryItemsResumed: (...args: unknown[]) => testContext.markAutomationRecoveryItemsResumedMock(...args),
    saveRecoveredItems: (...args: unknown[]) => testContext.saveRecoveredItemsMock(...args),
}));

vi.mock('../batchQueueStore', () => ({
    useBatchQueueStore: {
        getState: () => ({
            enqueueRecoveredItems: (...args: unknown[]) => testContext.enqueueRecoveredItemsMock(...args),
        }),
    },
}));

vi.mock('../automationStore', () => ({
    useAutomationStore: {
        getState: () => ({
            markRecoveryItemDiscarded: (...args: unknown[]) => testContext.markRecoveryItemDiscardedMock(...args),
        }),
    },
}));

vi.mock('../../services/taskLedgerRuntime', () => ({
    buildRecoveryTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(testContext.buildRecoveryTaskLedgerRecordMock, undefined, args),
    upsertTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(testContext.upsertTaskLedgerRecordMock, undefined, args),
    patchTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(testContext.patchTaskLedgerRecordMock, undefined, args),
    createRecoveryTaskLedgerId: (id: string) => `recovery-${id}`,
}));

import { useRecoveryStore } from '../recoveryStore';

describe('recoveryStore', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        testContext.loadRecoverySnapshotMock.mockResolvedValue({
            version: 1,
            updatedAt: 100,
            items: [],
        });
        testContext.saveRecoveredItemsMock.mockResolvedValue(undefined);
        testContext.markRecoveryItemDiscardedMock.mockResolvedValue(undefined);
        useRecoveryStore.setState({
            items: [],
            updatedAt: null,
            isLoaded: false,
            isBusy: false,
            error: null,
        });
    });

    it('loads an empty snapshot without showing recovery items', async () => {
        await useRecoveryStore.getState().loadRecovery();

        expect(useRecoveryStore.getState()).toEqual(expect.objectContaining({
            items: [],
            isLoaded: true,
            error: null,
        }));
    });

    it('mirrors pending recovery items into the task ledger', async () => {
        const recoveryItem = {
            id: 'recovery-1',
            filename: 'meeting.wav',
            filePath: 'C:\\watch\\meeting.wav',
            source: 'batch_import' as const,
            resolution: 'pending' as const,
            progress: 33,
            segments: [],
            projectId: null,
            lastKnownStage: 'transcribing' as const,
            updatedAt: 100,
            hasSourceFile: true,
            canResume: true,
        };
        testContext.loadRecoverySnapshotMock.mockResolvedValueOnce({
            version: 1,
            updatedAt: 100,
            items: [recoveryItem],
        });

        await useRecoveryStore.getState().loadRecovery();

        expect(testContext.upsertTaskLedgerRecordMock).toHaveBeenCalledWith(expect.objectContaining({
            id: 'recovery-recovery-1',
            status: 'recoverable',
            title: 'meeting.wav',
        }));
    });

    it('re-enqueues resumable items and clears the persisted recovery snapshot', async () => {
        const snapshotItems = [
            {
                id: 'recovery-1',
                filename: 'meeting.wav',
                filePath: 'C:\\watch\\meeting.wav',
                source: 'batch_import' as const,
                resolution: 'pending' as const,
                progress: 33,
                segments: [],
                projectId: null,
                lastKnownStage: 'transcribing' as const,
                updatedAt: 100,
                hasSourceFile: true,
                canResume: true,
                historyId: 'history-1',
            },
        ];

        useRecoveryStore.setState({
            items: snapshotItems,
            updatedAt: 100,
            isLoaded: true,
            isBusy: false,
            error: null,
        });

        await useRecoveryStore.getState().resumeAll();

        expect(testContext.markAutomationRecoveryItemsResumedMock).toHaveBeenCalledWith(snapshotItems);
        expect(testContext.enqueueRecoveredItemsMock).toHaveBeenCalledWith(snapshotItems);
        expect(testContext.saveRecoveredItemsMock).toHaveBeenCalledWith([]);
        expect(useRecoveryStore.getState().items).toEqual([]);
    });

    it('resumes one recovery item from the task center', async () => {
        const recoveryItem = {
            id: 'recovery-single',
            filename: 'single.wav',
            filePath: 'C:\\watch\\single.wav',
            source: 'batch_import' as const,
            resolution: 'pending' as const,
            progress: 20,
            segments: [],
            projectId: null,
            lastKnownStage: 'queued' as const,
            updatedAt: 100,
            hasSourceFile: true,
            canResume: true,
        };
        useRecoveryStore.setState({
            items: [recoveryItem],
            updatedAt: 100,
            isLoaded: true,
            isBusy: false,
            error: null,
        });

        await useRecoveryStore.getState().resumeItem('recovery-single');

        expect(testContext.enqueueRecoveredItemsMock).toHaveBeenCalledWith([recoveryItem]);
        expect(testContext.patchTaskLedgerRecordMock).toHaveBeenCalledWith('recovery-recovery-single', expect.objectContaining({
            status: 'succeeded',
            progress: 100,
        }));
        expect(testContext.saveRecoveredItemsMock).toHaveBeenCalledWith([]);
        expect(useRecoveryStore.getState().items).toEqual([]);
    });

    it('blocks Resume All when a source file is missing', async () => {
        useRecoveryStore.setState({
            items: [
                {
                    id: 'recovery-missing',
                    filename: 'missing.wav',
                    filePath: 'C:\\missing.wav',
                    source: 'batch_import',
                    resolution: 'pending',
                    progress: 10,
                    segments: [],
                    projectId: null,
                    lastKnownStage: 'queued',
                    updatedAt: 100,
                    hasSourceFile: false,
                    canResume: false,
                },
            ],
            updatedAt: 100,
            isLoaded: true,
            isBusy: false,
            error: null,
        });

        await expect(useRecoveryStore.getState().resumeAll()).rejects.toThrow(
            'Discard missing source files before resuming recovery.',
        );

        expect(testContext.enqueueRecoveredItemsMock).not.toHaveBeenCalled();
    });

    it('discards automation recovery items and persists the remaining list', async () => {
        const automationItem = {
            id: 'recovery-automation-1',
            filename: 'automation.wav',
            filePath: 'C:\\watch\\automation.wav',
            source: 'automation' as const,
            resolution: 'pending' as const,
            progress: 44,
            segments: [],
            projectId: null,
            lastKnownStage: 'transcribing' as const,
            updatedAt: 100,
            hasSourceFile: true,
            canResume: true,
            automationRuleId: 'rule-1',
            sourceFingerprint: 'fp-1',
        };
        const batchItem = {
            id: 'recovery-batch-1',
            filename: 'batch.wav',
            filePath: 'C:\\watch\\batch.wav',
            source: 'batch_import' as const,
            resolution: 'pending' as const,
            progress: 12,
            segments: [],
            projectId: null,
            lastKnownStage: 'queued' as const,
            updatedAt: 101,
            hasSourceFile: true,
            canResume: true,
        };

        useRecoveryStore.setState({
            items: [automationItem, batchItem],
            updatedAt: 100,
            isLoaded: true,
            isBusy: false,
            error: null,
        });

        await useRecoveryStore.getState().discardItem('recovery-automation-1');

        expect(testContext.markRecoveryItemDiscardedMock).toHaveBeenCalledWith(automationItem);
        expect(testContext.saveRecoveredItemsMock).toHaveBeenCalledWith([batchItem]);
        expect(useRecoveryStore.getState().items).toEqual([batchItem]);
    });
});
