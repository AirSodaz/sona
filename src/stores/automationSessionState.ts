import {
  createAutomationFingerprint,
  normalizeAutomationPath,
} from '../services/automationService';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeBlockReason,
  AutomationRuntimeState,
} from '../types/automation';
import type {
  AutomationRuntimeCandidatePayload,
  AutomationRuntimePathCollectionResult,
  AutomationRuntimeReplaceResult,
} from '../services/automationRuntimeService';
import type { AutomationTaskSettledPayload } from '../services/automationRuntimeBridge';
import type { RecoveryItemStage } from '../types/recovery';

const RETRY_SOURCE_MISSING_SIZE = 0;
const RETRY_SOURCE_MISSING_MTIME_MS = 0;

export type AutomationNotificationKind = 'failure' | 'success';

export interface AutomationSessionNotification {
  id: string;
  kind: AutomationNotificationKind;
  ruleId: string;
  ruleName: string;
  count: number;
  latestFilePath?: string;
  latestStage?: RecoveryItemStage;
  latestMessage?: string;
  createdAt: number;
  updatedAt: number;
  retryable: boolean;
  waveActive?: boolean;
}

export function getAutomationFailureNotificationId(ruleId: string): string {
  return `automation-failure-${ruleId}`;
}

export function hasRetryableFailures(ruleId: string, entries: AutomationProcessedEntry[]): boolean {
  return entries.some((entry) => entry.ruleId === ruleId && entry.status === 'error');
}

export function upsertFailureNotification(
  notifications: AutomationSessionNotification[],
  {
    ruleId,
    ruleName,
    filePath,
    stage,
    message,
    retryable,
    occurredAt = Date.now(),
  }: {
    ruleId: string;
    ruleName: string;
    filePath?: string;
    stage?: RecoveryItemStage;
    message?: string;
    retryable: boolean;
    occurredAt?: number;
  },
): AutomationSessionNotification[] {
  const notificationId = getAutomationFailureNotificationId(ruleId);
  const existing = notifications.find((notification) => notification.id === notificationId);
  const nextNotification: AutomationSessionNotification = {
    id: notificationId,
    kind: 'failure',
    ruleId,
    ruleName,
    count: (existing?.count || 0) + 1,
    latestFilePath: filePath ?? existing?.latestFilePath,
    latestStage: stage ?? existing?.latestStage,
    latestMessage: message ?? existing?.latestMessage,
    createdAt: existing?.createdAt ?? occurredAt,
    updatedAt: occurredAt,
    retryable,
  };

  return [
    nextNotification,
    ...notifications.filter((notification) => notification.id !== notificationId),
  ];
}

export function appendOrMergeSuccessNotification(
  notifications: AutomationSessionNotification[],
  {
    ruleId,
    ruleName,
    filePath,
    stage,
    occurredAt = Date.now(),
    waveActive,
    nextSuccessNotificationId,
  }: {
    ruleId: string;
    ruleName: string;
    filePath?: string;
    stage?: RecoveryItemStage;
    occurredAt?: number;
    waveActive: boolean;
    nextSuccessNotificationId: () => string;
  },
): AutomationSessionNotification[] {
  const existing = notifications.find((notification) => (
    notification.kind === 'success'
    && notification.ruleId === ruleId
    && notification.waveActive
  ));

  if (!existing) {
    return [
      {
        id: nextSuccessNotificationId(),
        kind: 'success',
        ruleId,
        ruleName,
        count: 1,
        latestFilePath: filePath,
        latestStage: stage,
        createdAt: occurredAt,
        updatedAt: occurredAt,
        retryable: false,
        waveActive,
      },
      ...notifications,
    ];
  }

  return notifications.map((notification) => (
    notification.id === existing.id
      ? {
        ...notification,
        ruleName,
        count: notification.count + 1,
        latestFilePath: filePath ?? notification.latestFilePath,
        latestStage: stage ?? notification.latestStage,
        updatedAt: occurredAt,
        waveActive,
      }
      : notification
  ));
}

export function removeRuleNotifications(
  notifications: AutomationSessionNotification[],
  ruleId: string,
  kind?: AutomationNotificationKind,
): AutomationSessionNotification[] {
  return notifications.filter((notification) => (
    notification.ruleId !== ruleId
    || (kind && notification.kind !== kind)
  ));
}

function resolveRuntimeOverride<K extends keyof AutomationRuntimeState>(
  overrides: Partial<AutomationRuntimeState>,
  key: K,
  fallback: AutomationRuntimeState[K],
): AutomationRuntimeState[K] {
  return Object.prototype.hasOwnProperty.call(overrides, key)
    ? overrides[key] as AutomationRuntimeState[K]
    : fallback;
}

export function deriveRuntimeState(
  ruleId: string,
  entries: AutomationProcessedEntry[],
  existing: AutomationRuntimeState | undefined,
  overrides: Partial<AutomationRuntimeState> = {},
): AutomationRuntimeState {
  const ruleEntries = entries
    .filter((entry) => entry.status !== 'discarded')
    .filter((entry) => entry.ruleId === ruleId)
    .sort((a, b) => b.processedAt - a.processedAt);
  const latest = ruleEntries[0];
  const failureCount = ruleEntries.filter((entry) => entry.status === 'error').length;

  return {
    ruleId,
    status: resolveRuntimeOverride(overrides, 'status', existing?.status ?? 'stopped'),
    lastScanAt: resolveRuntimeOverride(overrides, 'lastScanAt', existing?.lastScanAt),
    lastCandidateAt: resolveRuntimeOverride(overrides, 'lastCandidateAt', existing?.lastCandidateAt),
    lastQueuedAt: resolveRuntimeOverride(overrides, 'lastQueuedAt', existing?.lastQueuedAt),
    lastBlockedAt: resolveRuntimeOverride(overrides, 'lastBlockedAt', existing?.lastBlockedAt),
    lastBlockedReason: resolveRuntimeOverride(overrides, 'lastBlockedReason', existing?.lastBlockedReason),
    lastBlockedFilePath: resolveRuntimeOverride(overrides, 'lastBlockedFilePath', existing?.lastBlockedFilePath),
    lastProcessedAt: resolveRuntimeOverride(overrides, 'lastProcessedAt', latest?.processedAt ?? existing?.lastProcessedAt),
    lastResult: resolveRuntimeOverride(
      overrides,
      'lastResult',
      latest ? (latest.status === 'complete' ? 'success' : 'error') : existing?.lastResult,
    ),
    lastResultMessage: resolveRuntimeOverride(
      overrides,
      'lastResultMessage',
      latest?.errorMessage ?? existing?.lastResultMessage,
    ),
    lastProcessedFilePath: resolveRuntimeOverride(
      overrides,
      'lastProcessedFilePath',
      latest?.filePath ?? existing?.lastProcessedFilePath,
    ),
    failureCount: resolveRuntimeOverride(overrides, 'failureCount', failureCount),
  };
}

export function rebuildRuntimeStates(
  rules: AutomationRule[],
  entries: AutomationProcessedEntry[],
  current: Record<string, AutomationRuntimeState>,
): Record<string, AutomationRuntimeState> {
  return rules.reduce<Record<string, AutomationRuntimeState>>((acc, rule) => {
    acc[rule.id] = deriveRuntimeState(rule.id, entries, current[rule.id], {
      status: current[rule.id]?.status ?? (rule.enabled ? 'watching' : 'stopped'),
    });
    return acc;
  }, {});
}

export function applyRuntimeFailureState(
  current: {
    processedEntries: AutomationProcessedEntry[];
    runtimeStates: Record<string, AutomationRuntimeState>;
    notifications: AutomationSessionNotification[];
  },
  {
    ruleId,
    ruleName,
    message,
    filePath,
    stage,
    lastScanAt,
  }: {
    ruleId: string;
    ruleName: string;
    message: string;
    filePath?: string;
    stage?: RecoveryItemStage;
    lastScanAt?: number;
  },
) {
  return {
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        status: 'error',
        lastScanAt,
        lastResult: 'error',
        lastResultMessage: message,
        lastProcessedFilePath: filePath ?? current.runtimeStates[ruleId]?.lastProcessedFilePath,
      }),
    },
    notifications: upsertFailureNotification(current.notifications, {
      ruleId,
      ruleName,
      filePath,
      stage,
      message,
      retryable: hasRetryableFailures(ruleId, current.processedEntries),
      occurredAt: Date.now(),
    }),
  };
}

export function applyRuntimeBlockState(
  current: {
    processedEntries: AutomationProcessedEntry[];
    runtimeStates: Record<string, AutomationRuntimeState>;
  },
  {
    ruleId,
    filePath,
    reason,
    occurredAt = Date.now(),
  }: {
    ruleId: string;
    filePath: string;
    reason: AutomationRuntimeBlockReason;
    occurredAt?: number;
  },
) {
  return {
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        lastCandidateAt: occurredAt,
        lastBlockedAt: occurredAt,
        lastBlockedReason: reason,
        lastBlockedFilePath: filePath,
      }),
    },
  };
}

export function applyRuntimeQueuedState(
  current: {
    processedEntries: AutomationProcessedEntry[];
    runtimeStates: Record<string, AutomationRuntimeState>;
  },
  {
    ruleId,
    occurredAt = Date.now(),
  }: {
    ruleId: string;
    occurredAt?: number;
  },
) {
  return {
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        lastCandidateAt: occurredAt,
        lastQueuedAt: occurredAt,
      }),
    },
  };
}

export function getUniqueFilePaths(paths: string[]): string[] {
  const pathMap = new Map<string, string>();
  paths.forEach((filePath) => {
    const normalizedPath = normalizeAutomationPath(filePath);
    if (!pathMap.has(normalizedPath)) {
      pathMap.set(normalizedPath, filePath);
    }
  });

  return [...pathMap.values()];
}

export function buildRetryFailureEntry(
  ruleId: string,
  filePath: string,
  message: string,
  processedAt: number,
): AutomationProcessedEntry {
  return {
    ruleId,
    filePath,
    sourceFingerprint: createAutomationFingerprint(
      filePath,
      RETRY_SOURCE_MISSING_SIZE,
      RETRY_SOURCE_MISSING_MTIME_MS,
    ),
    size: RETRY_SOURCE_MISSING_SIZE,
    mtimeMs: RETRY_SOURCE_MISSING_MTIME_MS,
    status: 'error',
    processedAt,
    errorMessage: message,
  };
}

export function buildRetryBlockedEntry(
  ruleId: string,
  candidate: AutomationRuntimeCandidatePayload,
  message: string,
  processedAt: number,
): AutomationProcessedEntry {
  return {
    ruleId,
    filePath: candidate.filePath,
    sourceFingerprint: candidate.sourceFingerprint,
    size: candidate.size,
    mtimeMs: candidate.mtimeMs,
    status: 'error',
    processedAt,
    errorMessage: message,
  };
}

export function getRetryFailureMessage(result: AutomationRuntimePathCollectionResult): string {
  switch (result.outcome) {
    case 'missing':
      return 'Source file is no longer available for retry.';
    case 'unsupported':
    case 'excluded':
    case 'not_file':
      return 'Source file is no longer eligible for retry.';
    case 'error':
      return result.error
        ? `Retry source check failed: ${result.error}`
        : 'Retry source check failed.';
    default:
      return 'Source file is no longer available for retry.';
  }
}

export function getRetryBlockedMessage(reason: AutomationRuntimeBlockReason): string {
  switch (reason) {
    case 'recovery_blocked':
      return 'File is currently blocked by recovery state.';
    case 'project_missing':
      return 'Project not found.';
    case 'already_processed':
      return 'File has already been processed.';
    case 'already_pending':
      return 'File is already pending.';
    case 'retry_source_missing':
    default:
      return 'Source file is no longer available for retry.';
  }
}

interface AutomationSessionStateSnapshot {
  processedEntries: AutomationProcessedEntry[];
  runtimeStates: Record<string, AutomationRuntimeState>;
  notifications: AutomationSessionNotification[];
}

interface AutomationSessionRuleStateSnapshot extends AutomationSessionStateSnapshot {
  rules: AutomationRule[];
}

export function applyRetryFailureResults(
  current: AutomationSessionRuleStateSnapshot,
  rule: AutomationRule,
  results: AutomationRuntimePathCollectionResult[],
): AutomationSessionStateSnapshot {
  const failureResults = results.filter((result) => result.outcome !== 'candidate');
  if (failureResults.length === 0) {
    return current;
  }

  const entriesToAdd = failureResults.map((result, index) => (
    buildRetryFailureEntry(
      rule.id,
      result.filePath,
      getRetryFailureMessage(result),
      Date.now() + index,
    )
  ));
  const nextEntries = [
    ...current.processedEntries.filter((entry) => !(
      entry.ruleId === rule.id
      && entriesToAdd.some((candidate) => candidate.sourceFingerprint === entry.sourceFingerprint)
    )),
    ...entriesToAdd,
  ].sort((a, b) => b.processedAt - a.processedAt);

  let notifications = current.notifications;
  let runtimeStates = rebuildRuntimeStates(current.rules, nextEntries, current.runtimeStates);

  failureResults.forEach((result, index) => {
    const occurredAt = entriesToAdd[index].processedAt;
    notifications = upsertFailureNotification(notifications, {
      ruleId: rule.id,
      ruleName: rule.name,
      filePath: result.filePath,
      message: entriesToAdd[index].errorMessage,
      retryable: true,
      occurredAt,
    });
    runtimeStates = {
      ...runtimeStates,
      [rule.id]: deriveRuntimeState(rule.id, nextEntries, runtimeStates[rule.id], {
        lastCandidateAt: occurredAt,
        lastBlockedAt: occurredAt,
        lastBlockedReason: 'retry_source_missing',
        lastBlockedFilePath: result.filePath,
      }),
    };
  });

  return {
    processedEntries: nextEntries,
    notifications,
    runtimeStates,
  };
}

export function applyRetryBlockedResults(
  current: AutomationSessionRuleStateSnapshot,
  rule: AutomationRule,
  results: Array<{
    candidate: AutomationRuntimeCandidatePayload;
    reason: AutomationRuntimeBlockReason;
  }>,
): AutomationSessionStateSnapshot {
  if (results.length === 0) {
    return current;
  }

  const entriesToAdd = results.map(({ candidate, reason }, index) => (
    buildRetryBlockedEntry(
      rule.id,
      candidate,
      getRetryBlockedMessage(reason),
      Date.now() + index,
    )
  ));
  const nextEntries = [
    ...current.processedEntries.filter((entry) => !(
      entry.ruleId === rule.id
      && entriesToAdd.some((candidate) => candidate.sourceFingerprint === entry.sourceFingerprint)
    )),
    ...entriesToAdd,
  ].sort((a, b) => b.processedAt - a.processedAt);

  let notifications = current.notifications;
  let runtimeStates = rebuildRuntimeStates(current.rules, nextEntries, current.runtimeStates);

  results.forEach(({ candidate, reason }, index) => {
    const occurredAt = entriesToAdd[index].processedAt;
    notifications = upsertFailureNotification(notifications, {
      ruleId: rule.id,
      ruleName: rule.name,
      filePath: candidate.filePath,
      message: entriesToAdd[index].errorMessage,
      retryable: true,
      occurredAt,
    });
    runtimeStates = {
      ...runtimeStates,
      [rule.id]: deriveRuntimeState(rule.id, nextEntries, runtimeStates[rule.id], {
        lastCandidateAt: occurredAt,
        lastBlockedAt: occurredAt,
        lastBlockedReason: reason,
        lastBlockedFilePath: candidate.filePath,
      }),
    };
  });

  return {
    processedEntries: nextEntries,
    notifications,
    runtimeStates,
  };
}

export function applyRuntimeReplaceResults(
  current: AutomationSessionRuleStateSnapshot,
  results: AutomationRuntimeReplaceResult[],
): Pick<AutomationSessionStateSnapshot, 'runtimeStates' | 'notifications'> {
  let runtimeStates = rebuildRuntimeStates(current.rules, current.processedEntries, current.runtimeStates);
  let notifications = current.notifications;

  results.forEach((result) => {
    const rule = current.rules.find((item) => item.id === result.ruleId);
    if (!rule) {
      return;
    }

    if (result.started) {
      runtimeStates = {
        ...runtimeStates,
        [rule.id]: deriveRuntimeState(rule.id, current.processedEntries, runtimeStates[rule.id], {
          status: 'watching',
          lastResultMessage: undefined,
        }),
      };
      return;
    }

    const nextFailureState = applyRuntimeFailureState(
      {
        processedEntries: current.processedEntries,
        runtimeStates,
        notifications,
      },
      {
        ruleId: rule.id,
        ruleName: rule.name,
        message: result.error || 'Automation runtime failed to start.',
      },
    );
    runtimeStates = nextFailureState.runtimeStates;
    notifications = nextFailureState.notifications;
  });

  return {
    runtimeStates,
    notifications,
  };
}

export function applyTaskSettledState(
  current: AutomationSessionRuleStateSnapshot,
  payload: AutomationTaskSettledPayload,
  options: {
    fallbackRuleName?: string;
    waveActive: boolean;
    nextSuccessNotificationId: () => string;
  },
): AutomationSessionStateSnapshot {
  const nextRule = current.rules.find((item) => item.id === payload.ruleId);
  const nextRuleName = nextRule?.name || options.fallbackRuleName || 'Automation';
  const shouldClearBlockedHint = (
    current.runtimeStates[payload.ruleId]?.lastBlockedFilePath === payload.filePath
    && (current.runtimeStates[payload.ruleId]?.lastBlockedAt ?? 0) <= payload.processedAt
  );
  const nextNotifications = payload.status === 'complete'
    ? appendOrMergeSuccessNotification(current.notifications, {
      ruleId: payload.ruleId,
      ruleName: nextRuleName,
      filePath: payload.filePath,
      stage: payload.stage,
      occurredAt: payload.processedAt,
      waveActive: options.waveActive,
      nextSuccessNotificationId: options.nextSuccessNotificationId,
    })
    : upsertFailureNotification(current.notifications, {
      ruleId: payload.ruleId,
      ruleName: nextRuleName,
      filePath: payload.filePath,
      stage: payload.stage,
      message: payload.errorMessage,
      retryable: hasRetryableFailures(payload.ruleId, current.processedEntries),
      occurredAt: payload.processedAt,
    });

  return {
    processedEntries: current.processedEntries,
    notifications: nextNotifications,
    runtimeStates: {
      ...current.runtimeStates,
      [payload.ruleId]: deriveRuntimeState(
        payload.ruleId,
        current.processedEntries,
        current.runtimeStates[payload.ruleId],
        {
          status: nextRule?.enabled ? 'watching' : 'stopped',
          lastProcessedAt: payload.processedAt,
          lastProcessedFilePath: payload.filePath,
          lastResult: payload.status === 'complete' ? 'success' : 'error',
          lastResultMessage: payload.errorMessage,
          ...(shouldClearBlockedHint
            ? {
              lastBlockedAt: undefined,
              lastBlockedReason: undefined,
              lastBlockedFilePath: undefined,
            }
            : {}),
        },
      ),
    },
  };
}
