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
        ensureAutomationStorageMock: vi.fn().mockResolvedValue(undefined),
        listFilesRecursivelyMock: vi.fn(),
        loadAutomationProcessedEntriesMock: vi.fn(),
        loadAutomationRulesMock: vi.fn(),
        saveAutomationProcessedEntriesMock: vi.fn().mockResolvedValue(undefined),
        saveAutomationRulesMock: vi.fn().mockResolvedValue(undefined),
        validateAutomationRuleForActivationMock: vi.fn(),
        waitForStableAutomationFileMock: vi.fn(),
        watchAutomationDirectoryMock: vi.fn(),
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
    ensureAutomationStorageMock,
    listFilesRecursivelyMock,
    loadAutomationProcessedEntriesMock,
    loadAutomationRulesMock,
    saveAutomationProcessedEntriesMock,
    saveAutomationRulesMock,
    validateAutomationRuleForActivationMock,
    waitForStableAutomationFileMock,
    watchAutomationDirectoryMock,
    projectRecord,
    projectState,
    configState,
} = testContext;

vi.mock('uuid', () => ({
    v4: () => 'automation-rule-new',
}));

vi.mock('../batchQueueStore', () => ({
    useBatchQueueStore: {
        getState: () => ({
            addFiles: testContext.addFilesMock,
        }),
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
    listFilesRecursively: testContext.listFilesRecursivelyMock,
    loadAutomationProcessedEntries: testContext.loadAutomationProcessedEntriesMock,
    loadAutomationRules: testContext.loadAutomationRulesMock,
    normalizeAutomationPath: vi.fn((value: string) => value.trim().replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase()),
    saveAutomationProcessedEntries: testContext.saveAutomationProcessedEntriesMock,
    saveAutomationRules: testContext.saveAutomationRulesMock,
    validateAutomationRuleForActivation: testContext.validateAutomationRuleForActivationMock,
    waitForStableAutomationFile: testContext.waitForStableAutomationFileMock,
    watchAutomationDirectory: testContext.watchAutomationDirectoryMock,
}));

import { __notifyAutomationTaskSettledForTests, useAutomationStore } from '../automationStore';

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

        listFilesRecursivelyMock.mockResolvedValue([]);
        loadAutomationProcessedEntriesMock.mockResolvedValue([]);
        loadAutomationRulesMock.mockResolvedValue([]);
        validateAutomationRuleForActivationMock.mockResolvedValue({ valid: true });
        waitForStableAutomationFileMock.mockResolvedValue({
            filePath: 'C:\\watch\\meeting.wav',
            size: 42,
            mtimeMs: 1000,
            sourceFingerprint: 'fp-1',
        });
        watchAutomationDirectoryMock.mockImplementation(async (_path: string, _recursive: boolean, _onEvent: (paths: string[]) => void) => vi.fn());

        await useAutomationStore.getState().stopAll();
        useAutomationStore.setState({
            rules: [],
            processedEntries: [],
            runtimeStates: {},
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
        listFilesRecursivelyMock.mockResolvedValue(['C:\\watch\\meeting.wav']);

        await useAutomationStore.getState().loadAndStart();
        await vi.advanceTimersByTimeAsync(250);

        expect(ensureAutomationStorageMock).toHaveBeenCalled();
        expect(watchAutomationDirectoryMock).toHaveBeenCalledWith(
            'C:\\watch',
            true,
            expect.any(Function),
        );
        expect(addFilesMock).toHaveBeenCalledWith(
            ['C:\\watch\\meeting.wav'],
            expect.objectContaining({
                origin: 'automation',
                automationRuleId: 'rule-1',
                automationRuleName: 'Meeting Inbox',
                projectId: projectRecord.id,
                sourceFingerprint: 'fp-1',
                exportFileNamePrefix: 'TEAM',
            }),
        );
    });

    it('keeps pending dedupe scoped to each rule so identical files can be processed by multiple rules', async () => {
        const ruleA = createRule({ id: 'rule-a', name: 'Rule A' });
        const ruleB = createRule({ id: 'rule-b', name: 'Rule B' });
        loadAutomationRulesMock.mockResolvedValue([ruleA, ruleB]);
        listFilesRecursivelyMock.mockResolvedValue(['C:\\watch\\meeting.wav']);

        await useAutomationStore.getState().loadAndStart();
        await vi.advanceTimersByTimeAsync(250);

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
            isLoaded: true,
            error: null,
        });
        listFilesRecursivelyMock.mockResolvedValue(['C:\\watch\\failed.wav']);
        waitForStableAutomationFileMock.mockResolvedValue({
            filePath: 'C:\\watch\\failed.wav',
            size: 8,
            mtimeMs: 10,
            sourceFingerprint: 'failed-fingerprint',
        });

        await useAutomationStore.getState().retryFailed(rule.id);
        await vi.advanceTimersByTimeAsync(250);

        expect(saveAutomationProcessedEntriesMock).toHaveBeenCalledWith([completeEntry]);
        expect(addFilesMock).toHaveBeenCalledWith(
            ['C:\\watch\\failed.wav'],
            expect.objectContaining({
                automationRuleId: rule.id,
                sourceFingerprint: 'failed-fingerprint',
            }),
        );
    });

    it('records task completion back into the processed manifest and runtime state', async () => {
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
    });
});
