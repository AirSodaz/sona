import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeState,
} from '../../types/automation';
import type { AutomationTaskSettledPayload } from '../../services/automationRuntimeBridge';
import {
  applyRetryBlockedResults,
  applyRetryFailureResults,
  applyRuntimeReplaceResults,
  applyTaskSettledState,
  type AutomationSessionNotification,
} from '../automationSessionState';

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
  return {
    id: 'rule-1',
    name: 'Automation Rule',
    projectId: 'project-1',
    presetId: 'meeting_notes',
    watchDirectory: 'C:\\watch',
    recursive: true,
    enabled: true,
    stageConfig: {
      autoPolish: false,
      autoTranslate: false,
      exportEnabled: false,
    },
    exportConfig: {
      directory: 'C:\\exports',
      format: 'txt',
      mode: 'original',
      prefix: '',
    },
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

function createRuntimeState(overrides: Partial<AutomationRuntimeState> = {}): AutomationRuntimeState {
  return {
    ruleId: 'rule-1',
    status: 'stopped',
    failureCount: 0,
    ...overrides,
  };
}

function createNotification(
  overrides: Partial<AutomationSessionNotification> = {},
): AutomationSessionNotification {
  return {
    id: 'notification-1',
    kind: 'success',
    ruleId: 'rule-1',
    ruleName: 'Automation Rule',
    count: 1,
    createdAt: 1,
    updatedAt: 1,
    retryable: false,
    ...overrides,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('automationSessionState', () => {
  it('recreates a retryable missing-source failure entry with retry_source_missing runtime state', () => {
    vi.spyOn(Date, 'now').mockReturnValue(5000);

    const rule = createRule();
    const nextState = applyRetryFailureResults(
      {
        rules: [rule],
        processedEntries: [],
        runtimeStates: {
          [rule.id]: createRuntimeState(),
        },
        notifications: [],
      },
      rule,
      [
        {
          filePath: 'C:\\watch\\missing.wav',
          outcome: 'missing',
        },
      ],
    );

    expect(nextState.processedEntries).toEqual([
      expect.objectContaining({
        ruleId: rule.id,
        filePath: 'C:\\watch\\missing.wav',
        size: 0,
        mtimeMs: 0,
        status: 'error',
        errorMessage: 'Source file is no longer available for retry.',
      }),
    ]);
    expect(nextState.notifications).toEqual([
      expect.objectContaining({
        kind: 'failure',
        ruleId: rule.id,
        latestFilePath: 'C:\\watch\\missing.wav',
        latestMessage: 'Source file is no longer available for retry.',
        retryable: true,
      }),
    ]);
    expect(nextState.runtimeStates[rule.id]).toEqual(expect.objectContaining({
      lastBlockedReason: 'retry_source_missing',
      lastBlockedFilePath: 'C:\\watch\\missing.wav',
      failureCount: 1,
    }));
  });

  it('recreates a blocked retry candidate as an error entry and preserves the block reason', () => {
    vi.spyOn(Date, 'now').mockReturnValue(6000);

    const rule = createRule();
    const nextState = applyRetryBlockedResults(
      {
        rules: [rule],
        processedEntries: [],
        runtimeStates: {
          [rule.id]: createRuntimeState(),
        },
        notifications: [],
      },
      rule,
      [
        {
          candidate: {
            ruleId: rule.id,
            filePath: 'C:\\watch\\blocked.wav',
            sourceFingerprint: 'fp-blocked',
            size: 12,
            mtimeMs: 34,
          },
          reason: 'recovery_blocked',
        },
      ],
    );

    expect(nextState.processedEntries).toEqual([
      expect.objectContaining({
        ruleId: rule.id,
        filePath: 'C:\\watch\\blocked.wav',
        sourceFingerprint: 'fp-blocked',
        size: 12,
        mtimeMs: 34,
        status: 'error',
        errorMessage: 'File is currently blocked by recovery state.',
      }),
    ]);
    expect(nextState.notifications).toEqual([
      expect.objectContaining({
        kind: 'failure',
        ruleId: rule.id,
        latestFilePath: 'C:\\watch\\blocked.wav',
        latestMessage: 'File is currently blocked by recovery state.',
        retryable: true,
      }),
    ]);
    expect(nextState.runtimeStates[rule.id]).toEqual(expect.objectContaining({
      lastBlockedReason: 'recovery_blocked',
      lastBlockedFilePath: 'C:\\watch\\blocked.wav',
      failureCount: 1,
    }));
  });

  it('marks started runtime rules as watching and converts failed starts into failure notifications', () => {
    const firstRule = createRule();
    const secondRule = createRule({ id: 'rule-2', name: 'Rule Two' });
    const nextState = applyRuntimeReplaceResults(
      {
        rules: [firstRule, secondRule],
        processedEntries: [],
        runtimeStates: {
          [firstRule.id]: createRuntimeState({ ruleId: firstRule.id }),
          [secondRule.id]: createRuntimeState({ ruleId: secondRule.id }),
        },
        notifications: [],
      },
      [
        { ruleId: firstRule.id, started: true },
        { ruleId: secondRule.id, started: false, error: 'Watcher failed to start.' },
      ],
    );

    expect(nextState.runtimeStates[firstRule.id]).toEqual(expect.objectContaining({
      status: 'watching',
      lastResultMessage: undefined,
    }));
    expect(nextState.runtimeStates[secondRule.id]).toEqual(expect.objectContaining({
      status: 'error',
      lastResult: 'error',
      lastResultMessage: 'Watcher failed to start.',
    }));
    expect(nextState.notifications).toEqual([
      expect.objectContaining({
        kind: 'failure',
        ruleId: secondRule.id,
        ruleName: secondRule.name,
        latestMessage: 'Watcher failed to start.',
      }),
    ]);
  });

  it('merges settled success notifications within an active wave and clears stale blocked hints', () => {
    const rule = createRule();
    const processedEntries: AutomationProcessedEntry[] = [
      {
        ruleId: rule.id,
        filePath: 'C:\\watch\\meeting.wav',
        sourceFingerprint: 'fp-success',
        size: 42,
        mtimeMs: 1000,
        status: 'complete',
        processedAt: 400,
      },
    ];
    const payload: AutomationTaskSettledPayload = {
      ruleId: rule.id,
      filePath: 'C:\\watch\\meeting.wav',
      sourceFingerprint: 'fp-success',
      size: 42,
      mtimeMs: 1000,
      status: 'complete',
      processedAt: 400,
      stage: 'exporting',
    };
    const nextState = applyTaskSettledState(
      {
        rules: [rule],
        processedEntries,
        runtimeStates: {
          [rule.id]: createRuntimeState({
            status: 'watching',
            lastBlockedAt: 390,
            lastBlockedReason: 'already_pending',
            lastBlockedFilePath: 'C:\\watch\\meeting.wav',
          }),
        },
        notifications: [
          createNotification({
            id: 'automation-success-rule-1-1',
            waveActive: true,
            latestFilePath: 'C:\\watch\\previous.wav',
          }),
        ],
      },
      payload,
      {
        waveActive: true,
        nextSuccessNotificationId: () => 'automation-success-rule-1-2',
      },
    );

    expect(nextState.notifications).toEqual([
      expect.objectContaining({
        id: 'automation-success-rule-1-1',
        kind: 'success',
        ruleId: rule.id,
        count: 2,
        latestFilePath: 'C:\\watch\\meeting.wav',
        latestStage: 'exporting',
        waveActive: true,
      }),
    ]);
    expect(nextState.runtimeStates[rule.id]).toEqual(expect.objectContaining({
      status: 'watching',
      lastResult: 'success',
      lastProcessedFilePath: 'C:\\watch\\meeting.wav',
      lastBlockedReason: undefined,
      lastBlockedFilePath: undefined,
    }));
  });
});
