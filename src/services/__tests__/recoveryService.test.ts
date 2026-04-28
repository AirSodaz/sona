import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BatchQueueItem } from '../../types/batchQueue';
import {
    clearAutomationRecoveryGuardEntry,
    flushRecoverySnapshotWrites,
    isAutomationRecoveryBlocked,
    loadRecoverySnapshot,
    markAutomationRecoveryItemsResumed,
    persistQueueRecoverySnapshot,
    resetRecoveryRuntimeForTests,
} from '../recoveryService';

const testContext = vi.hoisted(() => ({
    filePaths: new Set<string>(),
    directoryPaths: new Set<string>(),
    unknownPaths: new Set<string>(),
    pathStatusError: null as Error | null,
    storedRecoveryFile: '',
    writeTextFileMock: vi.fn(),
    readTextFileMock: vi.fn(),
    existsMock: vi.fn(),
    invokeMock: vi.fn(),
    mkdirMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
    invoke: (...args: unknown[]) => testContext.invokeMock(...args),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    BaseDirectory: { AppLocalData: 3 },
    exists: (...args: unknown[]) => testContext.existsMock(...args),
    mkdir: (...args: unknown[]) => testContext.mkdirMock(...args),
    readTextFile: (...args: unknown[]) => testContext.readTextFileMock(...args),
    writeTextFile: (...args: unknown[]) => testContext.writeTextFileMock(...args),
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        error: vi.fn(),
    },
}));

describe('recoveryService', () => {
    beforeEach(() => {
        resetRecoveryRuntimeForTests();
        testContext.filePaths = new Set<string>();
        testContext.directoryPaths = new Set<string>();
        testContext.unknownPaths = new Set<string>();
        testContext.pathStatusError = null;
        testContext.storedRecoveryFile = '';
        vi.clearAllMocks();

        testContext.existsMock.mockImplementation(async (path: string) => {
            if (path === 'recovery' || path === 'recovery/queue-recovery.json') {
                return testContext.storedRecoveryFile.length > 0;
            }
            return false;
        });
        testContext.mkdirMock.mockResolvedValue(undefined);
        testContext.readTextFileMock.mockImplementation(async () => testContext.storedRecoveryFile);
        testContext.writeTextFileMock.mockImplementation(async (_path: string, content: string) => {
            testContext.storedRecoveryFile = content;
        });
        testContext.invokeMock.mockImplementation(async (command: string, payload?: { paths?: string[] }) => {
            if (command !== 'get_path_statuses') {
                throw new Error(`Unexpected command: ${command}`);
            }

            if (testContext.pathStatusError) {
                throw testContext.pathStatusError;
            }

            return (payload?.paths ?? []).map((path) => ({
                path,
                kind: testContext.unknownPaths.has(path)
                    ? 'unknown'
                    : testContext.filePaths.has(path)
                        ? 'file'
                        : testContext.directoryPaths.has(path)
                            ? 'directory'
                            : 'missing',
                error: testContext.unknownPaths.has(path) ? 'Scope denied' : null,
            }));
        });
    });

    it('persists only pending and processing queue items', async () => {
        const queueItems: BatchQueueItem[] = [
            {
                id: 'pending-1',
                filename: 'pending.wav',
                filePath: 'C:\\pending.wav',
                status: 'pending',
                progress: 0,
                segments: [],
                projectId: null,
                lastKnownStage: 'queued',
            },
            {
                id: 'processing-1',
                filename: 'processing.wav',
                filePath: 'C:\\processing.wav',
                status: 'processing',
                progress: 48,
                segments: [],
                projectId: null,
                lastKnownStage: 'transcribing',
            },
            {
                id: 'complete-1',
                filename: 'complete.wav',
                filePath: 'C:\\complete.wav',
                status: 'complete',
                progress: 100,
                segments: [],
                projectId: null,
            },
            {
                id: 'error-1',
                filename: 'error.wav',
                filePath: 'C:\\error.wav',
                status: 'error',
                progress: 88,
                segments: [],
                projectId: null,
            },
        ];

        persistQueueRecoverySnapshot(queueItems, { immediate: true });
        await flushRecoverySnapshotWrites();

        const parsed = JSON.parse(testContext.storedRecoveryFile);
        expect(parsed.items).toHaveLength(2);
        expect(parsed.items.map((item: { id: string }) => item.id)).toEqual(['pending-1', 'processing-1']);
    });

    it('marks missing source files as discard-only when loading a saved snapshot', async () => {
        testContext.storedRecoveryFile = JSON.stringify({
            version: 1,
            updatedAt: 100,
            items: [
                {
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
                },
            ],
        });

        const snapshot = await loadRecoverySnapshot();

        expect(snapshot.items).toEqual([
            expect.objectContaining({
                id: 'recovery-1',
                hasSourceFile: false,
                canResume: false,
            }),
        ]);
    });

    it('keeps resumable recovery items when the runtime confirms the source file exists', async () => {
        testContext.storedRecoveryFile = JSON.stringify({
            version: 1,
            updatedAt: 100,
            items: [
                {
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
                    hasSourceFile: false,
                    canResume: false,
                },
            ],
        });
        testContext.filePaths.add('C:\\watch\\meeting.wav');

        const snapshot = await loadRecoverySnapshot();

        expect(snapshot.items).toEqual([
            expect.objectContaining({
                id: 'recovery-1',
                hasSourceFile: true,
                canResume: true,
            }),
        ]);
    });

    it('preserves the saved recovery flags when path validation falls back to unknown', async () => {
        testContext.storedRecoveryFile = JSON.stringify({
            version: 1,
            updatedAt: 100,
            items: [
                {
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
                },
            ],
        });
        testContext.pathStatusError = new Error('Scope denied');

        const snapshot = await loadRecoverySnapshot();

        expect(snapshot.items).toEqual([
            expect.objectContaining({
                id: 'recovery-1',
                hasSourceFile: true,
                canResume: true,
            }),
        ]);
    });

    it('keeps automation recovery items blocked until they settle', async () => {
        testContext.storedRecoveryFile = JSON.stringify({
            version: 1,
            updatedAt: 200,
            items: [
                {
                    id: 'recovery-automation-1',
                    filename: 'automation.wav',
                    filePath: 'C:\\watch\\automation.wav',
                    source: 'automation',
                    resolution: 'pending',
                    progress: 62,
                    segments: [],
                    projectId: null,
                    lastKnownStage: 'translating',
                    updatedAt: 200,
                    hasSourceFile: true,
                    canResume: true,
                    automationRuleId: 'rule-1',
                    automationRuleName: 'Inbox Rule',
                    sourceFingerprint: 'fp-automation-1',
                    fileStat: {
                        size: 42,
                        mtimeMs: 500,
                    },
                },
            ],
        });
        testContext.filePaths.add('C:\\watch\\automation.wav');

        const snapshot = await loadRecoverySnapshot();

        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(true);

        markAutomationRecoveryItemsResumed(snapshot.items);
        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(true);

        clearAutomationRecoveryGuardEntry('rule-1', 'fp-automation-1');
        expect(isAutomationRecoveryBlocked('rule-1', 'fp-automation-1')).toBe(false);
    });
});
