import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutomationRule } from '../../types/automation';

const testContext = vi.hoisted(() => {
    const projectRecord = {
        id: 'project-1',
        name: 'Team Sync',
        description: '',
        createdAt: 1,
        updatedAt: 1,
        defaults: {
            summaryTemplateId: 'general',
            translationLanguage: 'ja',
            polishPresetId: 'general',
            exportFileNamePrefix: 'TEAM',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
            enabledPolishKeywordSetIds: [],
            enabledSpeakerProfileIds: [],
        },
    };

    return {
        addFilesMock: vi.fn(),
        batchQueueState: {
            addFiles: vi.fn(),
            queueItems: [] as any[],
        },
        ensureAutomationStorageMock: vi.fn().mockResolvedValue(undefined),
        loadAutomationProcessedEntriesMock: vi.fn(),
        loadAutomationRulesMock: vi.fn(),
        clearAutomationRecoveryGuardEntryMock: vi.fn(),
        isAutomationRecoveryBlockedMock: vi.fn(),
        listenToAutomationRuntimeCandidatesMock: vi.fn(),
        runtimeCandidateHandler: null as ((payload: any) => void | Promise<void>) | null,
        replaceAutomationRuntimeRulesMock: vi.fn(),
        scanAutomationRuntimeRuleMock: vi.fn().mockResolvedValue(undefined),
        saveAutomationProcessedEntriesMock: vi.fn().mockResolvedValue(undefined),
        saveAutomationRulesMock: vi.fn().mockResolvedValue(undefined),
        validateAutomationRuleForActivationMock: vi.fn(),
        projectRecord,
        projectState: {
            activeProjectId: projectRecord.id,
            getActiveProject: vi.fn(() => projectRecord),
            getProjectById: vi.fn((projectId: string | null | undefined) => (
                projectId === projectRecord.id ? projectRecord : null
            )),
        },
        configState: {
            config: {
                offlineModelPath: 'C:\\models\\sensevoice',
                translationLanguage: 'en',
                polishCustomPresets: [],
            },
        },
    };
});

const {
    addFilesMock,
    batchQueueState,
    clearAutomationRecoveryGuardEntryMock,
    ensureAutomationStorageMock,
    isAutomationRecoveryBlockedMock,
    listenToAutomationRuntimeCandidatesMock,
    loadAutomationProcessedEntriesMock,
    loadAutomationRulesMock,
    replaceAutomationRuntimeRulesMock,
    scanAutomationRuntimeRuleMock,
    saveAutomationProcessedEntriesMock,
    saveAutomationRulesMock,
    validateAutomationRuleForActivationMock,
    projectRecord,
    projectState,
    configState,
} = testContext;

vi.mock('uuid', () => ({
    v4: () => 'automation-rule-new',
}));

vi.mock('../batchQueueStore', () => ({
    useBatchQueueStore: {
        getState: () => testContext.batchQueueState,
    },
}));

vi.mock('../configStore', () => ({
    useConfigStore: {
        getState: () => testContext.configState,
    },
}));

vi.mock('../projectStore', () => ({
    useProjectStore: {
        getState: () => testContext.projectState,
    },
}));

vi.mock('../../services/effectiveConfigService', () => ({
    resolveEffectiveConfig: vi.fn((config: any, project: any) => ({
        ...config,
        translationLanguage: project?.defaults.translationLanguage ?? config.translationLanguage,
    })),
}));

vi.mock('../../services/automationService', () => ({
    ensureAutomationStorage: testContext.ensureAutomationStorageMock,
    isPathInsideDirectory: vi.fn((filePath: string, directoryPath: string) => {
        const normalize = (value: string) => value.trim().replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
        const normalizedFile = normalize(filePath);
        const normalizedDirectory = normalize(directoryPath);
        return normalizedFile === normalizedDirectory || normalizedFile.startsWith(`${normalizedDirectory}\\`);
    }),
    loadAutomationProcessedEntries: testContext.loadAutomationProcessedEntriesMock,
    loadAutomationRules: testContext.loadAutomationRulesMock,
    saveAutomationProcessedEntries: testContext.saveAutomationProcessedEntriesMock,
    saveAutomationRules: testContext.saveAutomationRulesMock,
    validateAutomationRuleForActivation: testContext.validateAutomationRuleForActivationMock,
}));

vi.mock('../../services/automationRuntimeService', () => ({
    listenToAutomationRuntimeCandidates: testContext.listenToAutomationRuntimeCandidatesMock,
    replaceAutomationRuntimeRules: testContext.replaceAutomationRuntimeRulesMock,
    scanAutomationRuntimeRule: testContext.scanAutomationRuntimeRuleMock,
    toAutomationRuntimeRuleConfig: vi.fn((rule: any) => ({
        ruleId: rule.id,
        watchDirectory: rule.watchDirectory,
        recursive: rule.recursive,
        excludeDirectory: rule.exportConfig.directory,
        debounceMs: 250,
        stableWindowMs: 5000,
    })),
}));

vi.mock('../../services/recoveryService', () => ({
    clearAutomationRecoveryGuardEntry: testContext.clearAutomationRecoveryGuardEntryMock,
    isAutomationRecoveryBlocked: testContext.isAutomationRecoveryBlockedMock,
}));

import { __notifyAutomationTaskSettledForTests, useAutomationStore } from '../automationStore';

async function emitRuntimeCandidate(payload: any) {
    if (!testContext.runtimeCandidateHandler) {
        throw new Error('Runtime candidate listener not registered.');
    }
    await testContext.runtimeCandidateHandler(payload);
}

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
    return {
        id: 'rule-1',
        name: 'Meeting Inbox',
        projectId: projectRecord.id,
        presetId: 'meeting_notes',
        watchDirectory: 'C:\\watch',
        recursive: true,
        enabled: true,
        stageConfig: {
            autoPolish: true,
            autoTranslate: false,
            exportEnabled: true,
        },
        exportConfig: {
            directory: 'C:\\exports',
            format: 'txt',
            mode: 'original',
        },
        createdAt: 1,
        updatedAt: 1,
        ...overrides,
    };
}

describe('automationStore', () => {
    beforeEach(async () => {
        vi.useFakeTimers();
        vi.clearAllMocks();

        projectState.activeProjectId = projectRecord.id;
        projectState.getActiveProject.mockImplementation(() => projectRecord);
        projectState.getProjectById.mockImplementation((projectId: string | null | undefined) => (
            projectId === projectRecord.id ? projectRecord : null
        ));
        configState.config = {
            offlineModelPath: 'C:\\models\\sensevoice',
            translationLanguage: 'en',
            polishCustomPresets: [],
        };
        batchQueueState.addFiles = addFilesMock;
        batchQueueState.queueItems = [];

        loadAutomationProcessedEntriesMock.mockResolvedValue([]);
        loadAutomationRulesMock.mockResolvedValue([]);
        isAutomationRecoveryBlockedMock.mockReturnValue(false);
        testContext.runtimeCandidateHandler = null;
        listenToAutomationRuntimeCandidatesMock.mockImplementation(async (handler: (payload: any) => void | Promise<void>) => {
            testContext.runtimeCandidateHandler = handler;
            return vi.fn(() => {
                testContext.runtimeCandidateHandler = null;
            });
        });
        replaceAutomationRuntimeRulesMock.mockImplementation(async (rules: Array<{ ruleId: string }>) => (
            rules.map((rule) => ({
                ruleId: rule.ruleId,
                started: true,
                error: null,
            }))
        ));
        scanAutomationRuntimeRuleMock.mockResolvedValue(undefined);
        validateAutomationRuleForActivationMock.mockResolvedValue({ valid: true });

        await useAutomationStore.getState().stopAll();
        vi.clearAllMocks();
        useAutomationStore.setState({
            rules: [],
            processedEntries: [],
            runtimeStates: {},
            notifications: [],
            isLoaded: false,
            error: null,
        });
    });

    afterEach(async () => {
        await useAutomationStore.getState().stopAll();
        vi.useRealTimers();
    });

    it('restores enabled rules and queues matching files on the initial scan', async () => {
        const rule = createRule();
        loadAutomationRulesMock.mockResolvedValue([rule]);

        await useAutomationStore.getState().loadAndStart();
        await emitRuntimeCandidate({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-1',
            size: 42,
            mtimeMs: 1000,
        });

        expect(ensureAutomationStorageMock).toHaveBeenCalled();
        expect(replaceAutomationRuntimeRulesMock).toHaveBeenCalledWith([
            expect.objectContaining({
                ruleId: rule.id,
                watchDirectory: 'C:\\watch',
                recursive: true,
                excludeDirectory: 'C:\\exports',
            }),
        ]);
        expect(addFilesMock).toHaveBeenCalledWith(
            ['C:\\watch\\meeting.wav'],
            expect.objectContaining({
                origin: 'automation',
                automationRuleId: 'rule-1',
                automationRuleName: 'Meeting Inbox',
                projectId: projectRecord.id,
                sourceFingerprint: 'fp-1',
            }),
        );
    });

    it('skips queuing files that are currently blocked by recovery guard', async () => {
        const rule = createRule();
        loadAutomationRulesMock.mockResolvedValue([rule]);
        isAutomationRecoveryBlockedMock.mockReturnValue(true);

        await useAutomationStore.getState().loadAndStart();
        await emitRuntimeCandidate({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-1',
            size: 42,
            mtimeMs: 1000,
        });

        expect(addFilesMock).not.toHaveBeenCalled();
    });

    it('does not re-queue files that already exist in imported processed entries', async () => {
        const rule = createRule();
        loadAutomationRulesMock.mockResolvedValue([rule]);
        loadAutomationProcessedEntriesMock.mockResolvedValue([
            {
                ruleId: rule.id,
                filePath: 'C:\\watch\\meeting.wav',
                sourceFingerprint: 'fp-1',
                size: 42,
                mtimeMs: 1000,
                status: 'complete',
                processedAt: 99,
            },
        ]);

        await useAutomationStore.getState().loadAndStart();
        await emitRuntimeCandidate({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-1',
            size: 42,
            mtimeMs: 1000,
        });

        expect(addFilesMock).not.toHaveBeenCalled();
    });

    it('keeps pending dedupe scoped to each rule so identical files can be processed by multiple rules', async () => {
        const ruleA = createRule({ id: 'rule-a', name: 'Rule A' });
        const ruleB = createRule({ id: 'rule-b', name: 'Rule B' });
        loadAutomationRulesMock.mockResolvedValue([ruleA, ruleB]);

        await useAutomationStore.getState().loadAndStart();
        await emitRuntimeCandidate({
            ruleId: ruleA.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-1',
            size: 42,
            mtimeMs: 1000,
        });
        await emitRuntimeCandidate({
            ruleId: ruleB.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-1',
            size: 42,
            mtimeMs: 1000,
        });

        expect(addFilesMock).toHaveBeenCalledTimes(2);
        expect(addFilesMock.mock.calls[0]?.[1]).toEqual(expect.objectContaining({
            automationRuleId: 'rule-a',
            sourceFingerprint: 'fp-1',
        }));
        expect(addFilesMock.mock.calls[1]?.[1]).toEqual(expect.objectContaining({
            automationRuleId: 'rule-b',
            sourceFingerprint: 'fp-1',
        }));
    });

    it('validates an enabled rule before persisting it', async () => {
        validateAutomationRuleForActivationMock.mockResolvedValue({
            valid: false,
            message: 'Missing offline model.',
        });

        await expect(useAutomationStore.getState().saveRule({
            name: 'Invalid Rule',
            projectId: projectRecord.id,
            presetId: 'meeting_notes',
            watchDirectory: 'C:\\watch',
            recursive: false,
            enabled: true,
            stageConfig: {
                autoPolish: true,
                autoTranslate: false,
                exportEnabled: true,
            },
            exportConfig: {
                directory: 'C:\\exports',
                format: 'txt',
                mode: 'original',
            },
        })).rejects.toThrow('Missing offline model.');

        expect(saveAutomationRulesMock).not.toHaveBeenCalled();
    });

    it('records runtime validation failures as non-retryable notifications', async () => {
        const rule = createRule({ enabled: false });
        validateAutomationRuleForActivationMock.mockResolvedValue({
            valid: false,
            message: 'Translation model missing.',
        });

        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'stopped',
                    failureCount: 0,
                },
            },
            notifications: [],
            isLoaded: true,
            error: null,
        });

        await expect(useAutomationStore.getState().scanRuleNow(rule.id)).rejects.toThrow('Translation model missing.');

        expect(useAutomationStore.getState().notifications).toEqual([
            expect.objectContaining({
                id: 'automation-failure-rule-1',
                kind: 'failure',
                ruleId: rule.id,
                ruleName: rule.name,
                count: 1,
                latestMessage: 'Translation model missing.',
                retryable: false,
            }),
        ]);
    });

    it('clears failed entries on retry and schedules a fresh scan', async () => {
        const rule = createRule({ enabled: false });
        const failedEntry = {
            ruleId: rule.id,
            filePath: 'C:\\watch\\failed.wav',
            sourceFingerprint: 'failed-fingerprint',
            size: 8,
            mtimeMs: 10,
            status: 'error' as const,
            processedAt: 20,
            errorMessage: 'Network error',
        };
        const completeEntry = {
            ruleId: rule.id,
            filePath: 'C:\\watch\\done.wav',
            sourceFingerprint: 'done-fingerprint',
            size: 9,
            mtimeMs: 11,
            status: 'complete' as const,
            processedAt: 21,
        };

        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [failedEntry, completeEntry],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'stopped',
                    failureCount: 1,
                    lastResult: 'error',
                },
            },
            notifications: [
                {
                    id: 'automation-failure-rule-1',
                    kind: 'failure',
                    ruleId: rule.id,
                    ruleName: rule.name,
                    count: 1,
                    latestFilePath: 'C:\\watch\\failed.wav',
                    latestMessage: 'Network error',
                    createdAt: 20,
                    updatedAt: 20,
                    retryable: true,
                },
            ],
            isLoaded: true,
            error: null,
        });

        await useAutomationStore.getState().retryFailed(rule.id);
        await emitRuntimeCandidate({
            ruleId: rule.id,
            filePath: 'C:\\watch\\failed.wav',
            sourceFingerprint: 'failed-fingerprint',
            size: 8,
            mtimeMs: 10,
        });

        expect(saveAutomationProcessedEntriesMock).toHaveBeenCalledWith([completeEntry]);
        expect(scanAutomationRuntimeRuleMock).toHaveBeenCalledWith(expect.objectContaining({
            ruleId: rule.id,
        }));
        expect(addFilesMock).toHaveBeenCalledWith(
            ['C:\\watch\\failed.wav'],
            expect.objectContaining({
                automationRuleId: rule.id,
                sourceFingerprint: 'failed-fingerprint',
            }),
        );
        expect(useAutomationStore.getState().notifications).toEqual([]);
    });

    it('retries failure notifications through the rule-level retry flow', async () => {
        const rule = createRule({ enabled: false });
        const failedEntry = {
            ruleId: rule.id,
            filePath: 'C:\\watch\\failed.wav',
            sourceFingerprint: 'failed-fingerprint',
            size: 8,
            mtimeMs: 10,
            status: 'error' as const,
            processedAt: 20,
            errorMessage: 'Network error',
        };

        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [failedEntry],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'stopped',
                    failureCount: 1,
                    lastResult: 'error',
                },
            },
            notifications: [
                {
                    id: 'automation-failure-rule-1',
                    kind: 'failure',
                    ruleId: rule.id,
                    ruleName: rule.name,
                    count: 1,
                    latestFilePath: 'C:\\watch\\failed.wav',
                    latestStage: 'transcribing',
                    latestMessage: 'Network error',
                    createdAt: 20,
                    updatedAt: 20,
                    retryable: true,
                },
            ],
            isLoaded: true,
            error: null,
        });

        await useAutomationStore.getState().retryNotification('automation-failure-rule-1');
        await emitRuntimeCandidate({
            ruleId: rule.id,
            filePath: 'C:\\watch\\failed.wav',
            sourceFingerprint: 'failed-fingerprint',
            size: 8,
            mtimeMs: 10,
        });

        expect(saveAutomationProcessedEntriesMock).toHaveBeenCalledWith([]);
        expect(scanAutomationRuntimeRuleMock).toHaveBeenCalledWith(expect.objectContaining({
            ruleId: rule.id,
        }));
        expect(addFilesMock).toHaveBeenCalledWith(
            ['C:\\watch\\failed.wav'],
            expect.objectContaining({
                automationRuleId: rule.id,
                sourceFingerprint: 'failed-fingerprint',
            }),
        );
        expect(useAutomationStore.getState().notifications).toEqual([]);
    });

    it('records task completion back into the processed manifest and runtime state', async () => {
        const rule = createRule();
        batchQueueState.queueItems = [
            {
                id: 'queue-1',
                filename: 'meeting.wav',
                filePath: 'C:\\watch\\meeting.wav',
                status: 'processing',
                progress: 90,
                segments: [],
                projectId: projectRecord.id,
                origin: 'automation',
                automationRuleId: rule.id,
            },
        ];
        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'watching',
                    failureCount: 0,
                },
            },
            notifications: [],
            isLoaded: true,
            error: null,
        });

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-complete',
            size: 42,
            mtimeMs: 1000,
            status: 'complete',
            processedAt: 2000,
            historyId: 'history-1',
            exportPath: 'C:\\exports\\meeting.txt',
            stage: 'exporting',
        });

        expect(saveAutomationProcessedEntriesMock).toHaveBeenCalledWith([
            expect.objectContaining({
                ruleId: rule.id,
                sourceFingerprint: 'fp-complete',
                status: 'complete',
                exportPath: 'C:\\exports\\meeting.txt',
            }),
        ]);
        expect(useAutomationStore.getState().runtimeStates[rule.id]).toEqual(expect.objectContaining({
            status: 'watching',
            lastResult: 'success',
            lastProcessedFilePath: 'C:\\watch\\meeting.wav',
            failureCount: 0,
        }));
        expect(useAutomationStore.getState().notifications).toEqual([
            expect.objectContaining({
                kind: 'success',
                ruleId: rule.id,
                ruleName: rule.name,
                count: 1,
                latestFilePath: 'C:\\watch\\meeting.wav',
                latestStage: 'exporting',
            }),
        ]);
        expect(clearAutomationRecoveryGuardEntryMock).toHaveBeenCalledWith(rule.id, 'fp-complete');
    });

    it('merges settled file errors into one retryable failure notification per rule', async () => {
        const rule = createRule();
        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'watching',
                    failureCount: 0,
                },
            },
            notifications: [],
            isLoaded: true,
            error: null,
        });

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting.wav',
            sourceFingerprint: 'fp-error-1',
            size: 42,
            mtimeMs: 1000,
            status: 'error',
            processedAt: 2100,
            errorMessage: 'Translation failed',
            stage: 'translating',
        });

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\meeting-2.wav',
            sourceFingerprint: 'fp-error-2',
            size: 43,
            mtimeMs: 1001,
            status: 'error',
            processedAt: 2200,
            errorMessage: 'Export failed',
            stage: 'exporting',
        });

        expect(useAutomationStore.getState().notifications).toEqual([
            expect.objectContaining({
                id: 'automation-failure-rule-1',
                kind: 'failure',
                ruleId: rule.id,
                ruleName: rule.name,
                count: 2,
                latestFilePath: 'C:\\watch\\meeting-2.wav',
                latestStage: 'exporting',
                latestMessage: 'Export failed',
                retryable: true,
            }),
        ]);
    });

    it('aggregates completion notifications within one contiguous rule wave and starts a new notification after the wave drains', async () => {
        const rule = createRule();
        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'watching',
                    failureCount: 0,
                },
            },
            notifications: [],
            isLoaded: true,
            error: null,
        });

        batchQueueState.queueItems = [
            {
                id: 'queue-1',
                filename: 'file-1.wav',
                filePath: 'C:\\watch\\file-1.wav',
                status: 'complete',
                progress: 100,
                segments: [],
                projectId: projectRecord.id,
                origin: 'automation',
                automationRuleId: rule.id,
            },
            {
                id: 'queue-2',
                filename: 'file-2.wav',
                filePath: 'C:\\watch\\file-2.wav',
                status: 'processing',
                progress: 70,
                segments: [],
                projectId: projectRecord.id,
                origin: 'automation',
                automationRuleId: rule.id,
            },
        ];

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\file-1.wav',
            sourceFingerprint: 'fp-success-1',
            size: 42,
            mtimeMs: 1000,
            status: 'complete',
            processedAt: 3000,
            stage: 'transcribing',
        });

        batchQueueState.queueItems = [
            {
                id: 'queue-2',
                filename: 'file-2.wav',
                filePath: 'C:\\watch\\file-2.wav',
                status: 'complete',
                progress: 100,
                segments: [],
                projectId: projectRecord.id,
                origin: 'automation',
                automationRuleId: rule.id,
            },
        ];

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\file-2.wav',
            sourceFingerprint: 'fp-success-2',
            size: 43,
            mtimeMs: 1001,
            status: 'complete',
            processedAt: 3100,
            stage: 'exporting',
        });

        batchQueueState.queueItems = [
            {
                id: 'queue-3',
                filename: 'file-3.wav',
                filePath: 'C:\\watch\\file-3.wav',
                status: 'complete',
                progress: 100,
                segments: [],
                projectId: projectRecord.id,
                origin: 'automation',
                automationRuleId: rule.id,
            },
        ];

        await __notifyAutomationTaskSettledForTests({
            ruleId: rule.id,
            filePath: 'C:\\watch\\file-3.wav',
            sourceFingerprint: 'fp-success-3',
            size: 44,
            mtimeMs: 1002,
            status: 'complete',
            processedAt: 3200,
            stage: 'exporting',
        });

        const successNotifications = useAutomationStore.getState().notifications.filter((notification) => notification.kind === 'success');
        expect(successNotifications).toHaveLength(2);
        expect(successNotifications[0]).toEqual(expect.objectContaining({
            ruleId: rule.id,
            count: 1,
            latestFilePath: 'C:\\watch\\file-3.wav',
        }));
        expect(successNotifications[1]).toEqual(expect.objectContaining({
            ruleId: rule.id,
            count: 2,
            latestFilePath: 'C:\\watch\\file-2.wav',
        }));
    });

    it('removes session notifications when deleting a rule', async () => {
        const rule = createRule({ enabled: false });
        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'stopped',
                    failureCount: 0,
                },
            },
            notifications: [
                {
                    id: 'automation-failure-rule-1',
                    kind: 'failure',
                    ruleId: rule.id,
                    ruleName: rule.name,
                    count: 1,
                    createdAt: 1,
                    updatedAt: 1,
                    retryable: true,
                },
                {
                    id: 'automation-success-rule-1-1',
                    kind: 'success',
                    ruleId: rule.id,
                    ruleName: rule.name,
                    count: 2,
                    createdAt: 2,
                    updatedAt: 2,
                    retryable: false,
                },
            ],
            isLoaded: true,
            error: null,
        });

        await useAutomationStore.getState().deleteRule(rule.id);

        expect(useAutomationStore.getState().notifications).toEqual([]);
    });

    it('records discarded recovery items without counting them as failures', async () => {
        const rule = createRule();
        const successfulEntry = {
            ruleId: rule.id,
            filePath: 'C:\\watch\\done.wav',
            sourceFingerprint: 'fp-success',
            size: 10,
            mtimeMs: 12,
            status: 'complete' as const,
            processedAt: 100,
        };

        useAutomationStore.setState({
            rules: [rule],
            processedEntries: [successfulEntry],
            runtimeStates: {
                [rule.id]: {
                    ruleId: rule.id,
                    status: 'watching',
                    failureCount: 0,
                    lastResult: 'success',
                    lastProcessedAt: 100,
                },
            },
            notifications: [],
            isLoaded: true,
            error: null,
        });

        await useAutomationStore.getState().markRecoveryItemDiscarded({
            id: 'recovery-1',
            filename: 'meeting.wav',
            filePath: 'C:\\watch\\meeting.wav',
            source: 'automation',
            resolution: 'pending',
            progress: 50,
            segments: [],
            projectId: projectRecord.id,
            lastKnownStage: 'transcribing',
            updatedAt: 200,
            hasSourceFile: true,
            canResume: true,
            automationRuleId: rule.id,
            automationRuleName: rule.name,
            sourceFingerprint: 'fp-discarded',
            fileStat: {
                size: 42,
                mtimeMs: 1000,
            },
        });

        expect(saveAutomationProcessedEntriesMock).toHaveBeenCalledWith([
            expect.objectContaining({
                sourceFingerprint: 'fp-discarded',
                status: 'discarded',
            }),
            successfulEntry,
        ]);
        expect(useAutomationStore.getState().runtimeStates[rule.id]).toEqual(expect.objectContaining({
            status: 'watching',
            lastResult: 'success',
            failureCount: 0,
            lastProcessedAt: 100,
        }));
    });
});
