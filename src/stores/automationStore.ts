import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import {
  createAutomationFingerprint,
  ensureAutomationStorage,
  isPathInsideDirectory,
  loadAutomationProcessedEntries,
  loadAutomationRules,
  normalizeAutomationPath,
  saveAutomationProcessedEntries,
  saveAutomationRules,
  validateAutomationRuleForActivation,
} from '../services/automationService';
import {
  collectAutomationRuntimeRulePaths,
  type AutomationRuntimeCandidatePayload,
  type AutomationRuntimePathCollectionResult,
  listenToAutomationRuntimeCandidates,
  replaceAutomationRuntimeRules,
  scanAutomationRuntimeRule,
  toAutomationRuntimeRuleConfig,
  type AutomationRuntimeReplaceResult,
} from '../services/automationRuntimeService';
import {
  emitAutomationTaskSettled,
  subscribeAutomationTaskSettled,
  type AutomationTaskSettledPayload,
} from '../services/automationRuntimeBridge';
import {
  clearAutomationRecoveryGuardEntry,
  isAutomationRecoveryBlocked,
} from '../services/recoveryService';
import { useBatchQueueStore } from './batchQueueStore';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeBlockReason,
  AutomationRuntimeState,
} from '../types/automation';
import type { RecoveredQueueItem, RecoveryItemStage } from '../types/recovery';
import { historyService } from '../services/historyService';
import { logger } from '../utils/logger';

const pendingFingerprints = new Set<string>();
let automationSuccessNotificationSequence = 0;
let automationRuntimeCandidateUnlisten: (() => void) | null = null;
const RETRY_SOURCE_MISSING_SIZE = 0;
const RETRY_SOURCE_MISSING_MTIME_MS = 0;

interface SaveRuleInput {
  id?: string;
  name: string;
  projectId: string;
  presetId: AutomationRule['presetId'];
  watchDirectory: string;
  recursive: boolean;
  stageConfig: AutomationRule['stageConfig'];
  exportConfig: AutomationRule['exportConfig'];
  enabled?: boolean;
}

type AutomationNotificationKind = 'failure' | 'success';

interface AutomationSessionNotification {
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

interface AutomationState {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
  runtimeStates: Record<string, AutomationRuntimeState>;
  notifications: AutomationSessionNotification[];
  isLoaded: boolean;
  error: string | null;
  loadAndStart: () => Promise<void>;
  saveRule: (input: SaveRuleInput) => Promise<AutomationRule>;
  deleteRule: (ruleId: string) => Promise<void>;
  toggleRuleEnabled: (ruleId: string, enabled: boolean) => Promise<void>;
  scanRuleNow: (ruleId: string) => Promise<void>;
  retryFailed: (ruleId: string) => Promise<void>;
  dismissNotification: (notificationId: string) => void;
  retryNotification: (notificationId: string) => Promise<void>;
  markRecoveryItemDiscarded: (item: RecoveredQueueItem) => Promise<void>;
  stopAll: () => Promise<void>;
}

interface HandleAutomationRuntimeCandidateOptions {
  suppressFailureNotification?: boolean;
}

type AutomationRuntimeCandidateHandleResult =
  | { status: 'queued' }
  | { status: 'blocked'; reason: AutomationRuntimeBlockReason }
  | { status: 'ignored' };

function nextAutomationSuccessNotificationId(ruleId: string): string {
  automationSuccessNotificationSequence += 1;
  return `automation-success-${ruleId}-${automationSuccessNotificationSequence}`;
}

function getAutomationFailureNotificationId(ruleId: string): string {
  return `automation-failure-${ruleId}`;
}

function hasRetryableFailures(ruleId: string, entries: AutomationProcessedEntry[]): boolean {
  return entries.some((entry) => entry.ruleId === ruleId && entry.status === 'error');
}

function hasAutomationItemsInFlight(ruleId: string): boolean {
  return useBatchQueueStore.getState().queueItems.some((item) => (
    item.origin === 'automation'
    && item.automationRuleId === ruleId
    && (item.status === 'pending' || item.status === 'processing')
  ));
}

function upsertFailureNotification(
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

function appendOrMergeSuccessNotification(
  notifications: AutomationSessionNotification[],
  {
    ruleId,
    ruleName,
    filePath,
    stage,
    occurredAt = Date.now(),
  }: {
    ruleId: string;
    ruleName: string;
    filePath?: string;
    stage?: RecoveryItemStage;
    occurredAt?: number;
  },
): AutomationSessionNotification[] {
  const waveActive = hasAutomationItemsInFlight(ruleId);
  const existing = notifications.find((notification) => (
    notification.kind === 'success'
    && notification.ruleId === ruleId
    && notification.waveActive
  ));

  if (!existing) {
    return [
      {
        id: nextAutomationSuccessNotificationId(ruleId),
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

function removeRuleNotifications(
  notifications: AutomationSessionNotification[],
  ruleId: string,
  kind?: AutomationNotificationKind,
): AutomationSessionNotification[] {
  return notifications.filter((notification) => (
    notification.ruleId !== ruleId
    || (kind && notification.kind !== kind)
  ));
}

function applyRuntimeFailureState(
  current: Pick<AutomationState, 'processedEntries' | 'runtimeStates' | 'notifications'>,
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
): Pick<AutomationState, 'runtimeStates' | 'notifications'> {
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

function applyRuntimeBlockState(
  current: Pick<AutomationState, 'processedEntries' | 'runtimeStates'>,
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
): Pick<AutomationState, 'runtimeStates'> {
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

function applyRuntimeQueuedState(
  current: Pick<AutomationState, 'processedEntries' | 'runtimeStates'>,
  {
    ruleId,
    occurredAt = Date.now(),
  }: {
    ruleId: string;
    occurredAt?: number;
  },
): Pick<AutomationState, 'runtimeStates'> {
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

function resolveRuntimeOverride<K extends keyof AutomationRuntimeState>(
  overrides: Partial<AutomationRuntimeState>,
  key: K,
  fallback: AutomationRuntimeState[K],
): AutomationRuntimeState[K] {
  return Object.prototype.hasOwnProperty.call(overrides, key)
    ? overrides[key] as AutomationRuntimeState[K]
    : fallback;
}

function deriveRuntimeState(
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
    lastResultMessage: resolveRuntimeOverride(overrides, 'lastResultMessage', latest?.errorMessage ?? existing?.lastResultMessage),
    lastProcessedFilePath: resolveRuntimeOverride(overrides, 'lastProcessedFilePath', latest?.filePath ?? existing?.lastProcessedFilePath),
    failureCount: resolveRuntimeOverride(overrides, 'failureCount', failureCount),
  };
}

function rebuildRuntimeStates(
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

function clearRulePendingFingerprints(ruleId: string) {
  for (const key of pendingFingerprints.values()) {
    if (key.startsWith(`${ruleId}::`)) {
      pendingFingerprints.delete(key);
    }
  }
}

function clearAllPendingFingerprints() {
  pendingFingerprints.clear();
}

function buildPendingFingerprintKey(ruleId: string, sourceFingerprint: string): string {
  return `${ruleId}::${sourceFingerprint}`;
}

function getUniqueFilePaths(paths: string[]): string[] {
  const pathMap = new Map<string, string>();
  paths.forEach((filePath) => {
    const normalizedPath = normalizeAutomationPath(filePath);
    if (!pathMap.has(normalizedPath)) {
      pathMap.set(normalizedPath, filePath);
    }
  });

  return [...pathMap.values()];
}

function buildRetryFailureEntry(
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

function buildRetryBlockedEntry(
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

function getRetryFailureMessage(result: AutomationRuntimePathCollectionResult): string {
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

function getRetryBlockedMessage(reason: AutomationRuntimeBlockReason): string {
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

async function handleAutomationRuntimeCandidatePayload(
  payload: AutomationRuntimeCandidatePayload,
  options?: HandleAutomationRuntimeCandidateOptions,
): Promise<AutomationRuntimeCandidateHandleResult> {
  const occurredAt = Date.now();
  const initialState = useAutomationStore.getState();
  const rule = initialState.rules.find((item) => item.id === payload.ruleId);
  if (!rule) {
    return { status: 'ignored' };
  }

  if (isPathInsideDirectory(payload.filePath, rule.exportConfig.directory)) {
    return { status: 'ignored' };
  }

  if (isAutomationRecoveryBlocked(payload.ruleId, payload.sourceFingerprint)) {
    useAutomationStore.setState((current) => ({
      ...applyRuntimeBlockState(current, {
        ruleId: payload.ruleId,
        filePath: payload.filePath,
        reason: 'recovery_blocked',
        occurredAt,
      }),
    }));
    return { status: 'blocked', reason: 'recovery_blocked' };
  }

  const latestState = useAutomationStore.getState();
  const latestRule = latestState.rules.find((item) => item.id === payload.ruleId);
  if (!latestRule) {
    return { status: 'ignored' };
  }

  if (latestState.processedEntries.some((entry) => (
    entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint
  ))) {
    useAutomationStore.setState((current) => ({
      ...applyRuntimeBlockState(current, {
        ruleId: payload.ruleId,
        filePath: payload.filePath,
        reason: 'already_processed',
        occurredAt,
      }),
    }));
    return { status: 'blocked', reason: 'already_processed' };
  }

  const pendingKey = buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint);
  if (pendingFingerprints.has(pendingKey)) {
    useAutomationStore.setState((current) => ({
      ...applyRuntimeBlockState(current, {
        ruleId: payload.ruleId,
        filePath: payload.filePath,
        reason: 'already_pending',
        occurredAt,
      }),
    }));
    return { status: 'blocked', reason: 'already_pending' };
  }

  const isInboxOrNone = latestRule.projectId === 'inbox' || latestRule.projectId === 'none';
  const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(latestRule.projectId);
  if (!project && !isInboxOrNone) {
    useAutomationStore.setState((current) => {
      if (options?.suppressFailureNotification) {
        const runtimeStatesWithError = {
          ...current.runtimeStates,
          [payload.ruleId]: deriveRuntimeState(
            payload.ruleId,
            current.processedEntries,
            current.runtimeStates[payload.ruleId],
            {
              status: 'error',
              lastResult: 'error',
              lastResultMessage: 'Project not found.',
              lastProcessedFilePath: payload.filePath,
            },
          ),
        };

        return {
          runtimeStates: applyRuntimeBlockState(
            {
              processedEntries: current.processedEntries,
              runtimeStates: runtimeStatesWithError,
            },
            {
              ruleId: payload.ruleId,
              filePath: payload.filePath,
              reason: 'project_missing',
              occurredAt,
            },
          ).runtimeStates,
        };
      }

      const nextFailureState = applyRuntimeFailureState(current, {
        ruleId: payload.ruleId,
        ruleName: latestRule.name,
        message: 'Project not found.',
        filePath: payload.filePath,
      });

      return {
        ...nextFailureState,
        ...applyRuntimeBlockState(
          {
            processedEntries: current.processedEntries,
            runtimeStates: nextFailureState.runtimeStates,
          },
          {
            ruleId: payload.ruleId,
            filePath: payload.filePath,
            reason: 'project_missing',
            occurredAt,
          },
        ),
      };
    });
    return { status: 'blocked', reason: 'project_missing' };
  }

  const effectiveConfig = {
    ...resolveEffectiveConfig(useConfigStore.getState().config, project),
    translationLanguage: latestRule.stageConfig.translationLanguage || 'en',
    polishPresetId: latestRule.stageConfig.polishPresetId || 'general',
  };
  pendingFingerprints.add(pendingKey);

  useBatchQueueStore.getState().addFiles([payload.filePath], {
    origin: 'automation',
    automationRuleId: latestRule.id,
    automationRuleName: latestRule.name,
    resolvedConfigSnapshot: effectiveConfig,
    exportConfig: latestRule.stageConfig.exportEnabled ? latestRule.exportConfig : null,
    stageConfig: latestRule.stageConfig,
    sourceFingerprint: payload.sourceFingerprint,
    projectId: isInboxOrNone ? null : latestRule.projectId,
    fileStat: {
      size: payload.size,
      mtimeMs: payload.mtimeMs,
    },
    exportFileNamePrefix: latestRule.exportConfig.prefix || '',
  });

  useAutomationStore.setState((current) => ({
    ...applyRuntimeQueuedState(current, {
      ruleId: payload.ruleId,
      occurredAt,
    }),
  }));

  return { status: 'queued' };
}

async function recordRetryFailures(
  rule: AutomationRule,
  results: AutomationRuntimePathCollectionResult[],
): Promise<void> {
  const failureResults = results.filter((result) => result.outcome !== 'candidate');
  if (failureResults.length === 0) {
    return;
  }

  const current = useAutomationStore.getState();
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

  await saveAutomationProcessedEntries(nextEntries);

  useAutomationStore.setState((state) => {
    let notifications = state.notifications;
    let runtimeStates = rebuildRuntimeStates(state.rules, nextEntries, state.runtimeStates);

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
  });
}

async function recordRetryBlockedCandidates(
  rule: AutomationRule,
  results: Array<{
    candidate: AutomationRuntimeCandidatePayload;
    reason: AutomationRuntimeBlockReason;
  }>,
): Promise<void> {
  if (results.length === 0) {
    return;
  }

  const current = useAutomationStore.getState();
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

  await saveAutomationProcessedEntries(nextEntries);

  useAutomationStore.setState((state) => {
    let notifications = state.notifications;
    let runtimeStates = rebuildRuntimeStates(state.rules, nextEntries, state.runtimeStates);

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
  });
}

async function validateRuleBeforeActivation(rule: AutomationRule): Promise<void> {
  const isInboxOrNone = rule.projectId === 'inbox' || rule.projectId === 'none';
  const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(rule.projectId);
  const validation = await validateAutomationRuleForActivation(
    rule,
    useConfigStore.getState().config,
    project,
  );

  if (!validation.valid) {
    throw new Error(validation.message || 'Automation rule validation failed.');
  }
}

function applyRuntimeReplaceResults(
  current: Pick<AutomationState, 'rules' | 'processedEntries' | 'runtimeStates' | 'notifications'>,
  results: AutomationRuntimeReplaceResult[],
) {
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

async function ensureAutomationRuntimeCandidateListener() {
  if (automationRuntimeCandidateUnlisten) {
    return;
  }

  automationRuntimeCandidateUnlisten = await listenToAutomationRuntimeCandidates(handleAutomationRuntimeCandidatePayload);
}

function clearAutomationRuntimeCandidateListener() {
  if (!automationRuntimeCandidateUnlisten) {
    return;
  }

  automationRuntimeCandidateUnlisten();
  automationRuntimeCandidateUnlisten = null;
}

async function syncAutomationRuntimeRules(options?: { throwForRuleId?: string }) {
  const state = useAutomationStore.getState();
  const enabledRules = state.rules.filter((rule) => rule.enabled);

  await ensureAutomationRuntimeCandidateListener();

  let results: AutomationRuntimeReplaceResult[];
  try {
    results = await replaceAutomationRuntimeRules(enabledRules.map((rule) => toAutomationRuntimeRuleConfig(rule)));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    useAutomationStore.setState((current) => {
      let runtimeStates = rebuildRuntimeStates(current.rules, current.processedEntries, current.runtimeStates);
      let notifications = current.notifications;

      current.rules.filter((rule) => rule.enabled).forEach((rule) => {
        const nextFailureState = applyRuntimeFailureState(
          {
            processedEntries: current.processedEntries,
            runtimeStates,
            notifications,
          },
          {
            ruleId: rule.id,
            ruleName: rule.name,
            message,
          },
        );
        runtimeStates = nextFailureState.runtimeStates;
        notifications = nextFailureState.notifications;
      });

      return {
        runtimeStates,
        notifications,
      };
    });
    throw error;
  }

  useAutomationStore.setState((current) => ({
    ...applyRuntimeReplaceResults(current, results),
  }));

  const targetFailure = options?.throwForRuleId
    ? results.find((result) => result.ruleId === options.throwForRuleId && !result.started)
    : undefined;

  if (targetFailure?.error) {
    throw new Error(targetFailure.error);
  }
}

async function scanRule(ruleId: string) {
  const state = useAutomationStore.getState();
  const rule = state.rules.find((item) => item.id === ruleId);
  if (!rule) {
    return;
  }

  const isInboxOrNone = rule.projectId === 'inbox' || rule.projectId === 'none';
  const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(rule.projectId);
  const validation = await validateAutomationRuleForActivation(rule, useConfigStore.getState().config, project);
  if (!validation.valid) {
    useAutomationStore.setState((current) => ({
      ...applyRuntimeFailureState(current, {
        ruleId,
        ruleName: rule.name,
        message: validation.message || 'Automation rule validation failed.',
        lastScanAt: Date.now(),
      }),
    }));
    throw new Error(validation.message || 'Automation rule validation failed.');
  }

  await ensureAutomationRuntimeCandidateListener();

  useAutomationStore.setState((current) => ({
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        status: 'scanning',
        lastScanAt: Date.now(),
      }),
    },
  }));

  try {
    await scanAutomationRuntimeRule(toAutomationRuntimeRuleConfig(rule));

    useAutomationStore.setState((current) => ({
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: rule.enabled ? 'watching' : 'stopped',
          lastScanAt: Date.now(),
        }),
      },
    }));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    useAutomationStore.setState((current) => ({
      ...applyRuntimeFailureState(current, {
        ruleId,
        ruleName: rule.name,
        message,
        lastScanAt: Date.now(),
      }),
    }));
    throw error;
  }
}

export const useAutomationStore = create<AutomationState>((set, get) => ({
  rules: [],
  processedEntries: [],
  runtimeStates: {},
  notifications: [],
  isLoaded: false,
  error: null,

  loadAndStart: async () => {
    await ensureAutomationStorage();
    await get().stopAll();

    const [rules, processedEntries] = await Promise.all([
      loadAutomationRules(),
      loadAutomationProcessedEntries(),
    ]);

    set({
      rules,
      processedEntries,
      runtimeStates: rebuildRuntimeStates(rules, processedEntries, {}),
      notifications: [],
      isLoaded: true,
      error: null,
    });

    await syncAutomationRuntimeRules();
  },

  saveRule: async (input) => {
    const now = Date.now();
    const state = get();
    const existing = input.id ? state.rules.find((rule) => rule.id === input.id) : null;

    const nextRule: AutomationRule = {
      id: existing?.id || uuidv4(),
      name: input.name.trim(),
      projectId: input.projectId,
      presetId: input.presetId,
      watchDirectory: input.watchDirectory.trim(),
      recursive: input.recursive,
      enabled: input.enabled ?? existing?.enabled ?? false,
      stageConfig: {
        ...input.stageConfig,
      },
      exportConfig: {
        ...input.exportConfig,
        directory: input.exportConfig.directory.trim(),
      },
      createdAt: existing?.createdAt || now,
      updatedAt: now,
    };

    if (nextRule.enabled) {
      await validateRuleBeforeActivation(nextRule);
    }

    const nextRules = existing
      ? state.rules.map((rule) => (rule.id === existing.id ? nextRule : rule))
      : [nextRule, ...state.rules];

    await saveAutomationRules(nextRules);
    set((current) => ({
      rules: nextRules,
      runtimeStates: rebuildRuntimeStates(nextRules, current.processedEntries, current.runtimeStates),
    }));

    if (nextRule.enabled) {
      await syncAutomationRuntimeRules({ throwForRuleId: nextRule.id });
    } else {
      clearRulePendingFingerprints(nextRule.id);
      await syncAutomationRuntimeRules();
    }

    return nextRule;
  },

  deleteRule: async (ruleId) => {
    const nextRules = get().rules.filter((rule) => rule.id !== ruleId);
    const nextProcessedEntries = get().processedEntries.filter((entry) => entry.ruleId !== ruleId);

    await Promise.all([
      saveAutomationRules(nextRules),
      saveAutomationProcessedEntries(nextProcessedEntries),
    ]);
    clearRulePendingFingerprints(ruleId);

    set((current) => {
      const nextRuntimeStates = { ...current.runtimeStates };
      delete nextRuntimeStates[ruleId];
      return {
        rules: nextRules,
        processedEntries: nextProcessedEntries,
        runtimeStates: nextRuntimeStates,
        notifications: removeRuleNotifications(current.notifications, ruleId),
      };
    });

    await syncAutomationRuntimeRules();
  },

  toggleRuleEnabled: async (ruleId, enabled) => {
    const state = get();
    const targetRule = state.rules.find((rule) => rule.id === ruleId);
    if (!targetRule) {
      return;
    }

    if (enabled) {
      await validateRuleBeforeActivation({
        ...targetRule,
        enabled: true,
      });
    }

    const nextRule = {
      ...targetRule,
      enabled,
      updatedAt: Date.now(),
    };
    const nextRules = state.rules.map((rule) => (rule.id === ruleId ? nextRule : rule));
    await saveAutomationRules(nextRules);
    set((current) => ({
      rules: nextRules,
      runtimeStates: rebuildRuntimeStates(nextRules, current.processedEntries, current.runtimeStates),
    }));

    if (enabled) {
      await syncAutomationRuntimeRules({ throwForRuleId: ruleId });
      return;
    }

    clearRulePendingFingerprints(ruleId);
    await syncAutomationRuntimeRules();
  },

  scanRuleNow: async (ruleId) => {
    await scanRule(ruleId);
  },

  retryFailed: async (ruleId) => {
    const state = get();
    const rule = state.rules.find((item) => item.id === ruleId);
    if (!rule) {
      return;
    }

    const failedEntries = state.processedEntries.filter((entry) => (
      entry.ruleId === ruleId && entry.status === 'error'
    ));
    if (failedEntries.length === 0) {
      set((current) => ({
        notifications: removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));
      return;
    }

    const isInboxOrNone = rule.projectId === 'inbox' || rule.projectId === 'none';
    const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(rule.projectId);
    const validation = await validateAutomationRuleForActivation(
      rule,
      useConfigStore.getState().config,
      project,
    );
    if (!validation.valid) {
      const lastScanAt = Date.now();
      useAutomationStore.setState((current) => ({
        ...applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message: validation.message || 'Automation rule validation failed.',
          lastScanAt,
        }),
      }));
      throw new Error(validation.message || 'Automation rule validation failed.');
    }

    const scanStartedAt = Date.now();
    set((current) => ({
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'scanning',
          lastScanAt: scanStartedAt,
        }),
      },
    }));

    const filePaths = getUniqueFilePaths(failedEntries.map((entry) => entry.filePath));

    try {
      const results = await collectAutomationRuntimeRulePaths(
        toAutomationRuntimeRuleConfig(rule),
        filePaths,
      );

      const nextProcessedEntries = state.processedEntries.filter((entry) => !(
        entry.ruleId === ruleId && entry.status === 'error'
      ));
      await saveAutomationProcessedEntries(nextProcessedEntries);
      set((current) => ({
        processedEntries: nextProcessedEntries,
        runtimeStates: {
          ...current.runtimeStates,
          [ruleId]: deriveRuntimeState(ruleId, nextProcessedEntries, current.runtimeStates[ruleId], {
            status: 'scanning',
            lastScanAt: scanStartedAt,
          }),
        },
        notifications: removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));

      const failureResults = results.filter((result) => result.outcome !== 'candidate');
      const candidateResults = results.filter((result) => (
        result.outcome === 'candidate' && result.candidate
      ));
      const blockedCandidateFailures: Array<{
        candidate: AutomationRuntimeCandidatePayload;
        reason: AutomationRuntimeBlockReason;
      }> = [];

      for (const result of candidateResults) {
        const candidate = result.candidate!;
        const handled = await handleAutomationRuntimeCandidatePayload(candidate, {
          suppressFailureNotification: true,
        });
        if (
          handled.status === 'blocked'
          && (handled.reason === 'recovery_blocked' || handled.reason === 'project_missing')
        ) {
          blockedCandidateFailures.push({
            candidate,
            reason: handled.reason,
          });
        }
      }

      await recordRetryFailures(rule, failureResults);
      await recordRetryBlockedCandidates(rule, blockedCandidateFailures);

      set((current) => {
        const runtime = current.runtimeStates[ruleId];
        if (runtime?.status !== 'scanning') {
          return {};
        }

        return {
          runtimeStates: {
            ...current.runtimeStates,
            [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, runtime, {
              status: rule.enabled ? 'watching' : 'stopped',
              lastScanAt: Date.now(),
            }),
          },
        };
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      useAutomationStore.setState((current) => ({
        ...applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message,
          lastScanAt: Date.now(),
        }),
      }));
      throw error;
    }
  },

  dismissNotification: (notificationId) => {
    set((current) => ({
      notifications: current.notifications.filter((notification) => notification.id !== notificationId),
    }));
  },

  retryNotification: async (notificationId) => {
    const notification = get().notifications.find((item) => item.id === notificationId);
    if (!notification || notification.kind !== 'failure' || !notification.retryable) {
      return;
    }

    await get().retryFailed(notification.ruleId);
  },

  markRecoveryItemDiscarded: async (item) => {
    if (!item.automationRuleId || !item.sourceFingerprint) {
      return;
    }

    const nextEntry: AutomationProcessedEntry = {
      ruleId: item.automationRuleId,
      filePath: item.filePath,
      sourceFingerprint: item.sourceFingerprint,
      size: item.fileStat?.size || 0,
      mtimeMs: item.fileStat?.mtimeMs || 0,
      status: 'discarded',
      processedAt: Date.now(),
      historyId: item.historyId,
      errorMessage: 'Discarded from recovery center.',
    };

    const nextProcessedEntries = [
      ...get().processedEntries.filter((entry) => !(
        entry.ruleId === nextEntry.ruleId
        && entry.sourceFingerprint === nextEntry.sourceFingerprint
      )),
      nextEntry,
    ].sort((a, b) => b.processedAt - a.processedAt);

    await saveAutomationProcessedEntries(nextProcessedEntries);
    clearAutomationRecoveryGuardEntry(item.automationRuleId, item.sourceFingerprint);
    set((current) => ({
      processedEntries: nextProcessedEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextProcessedEntries, current.runtimeStates),
    }));
  },

  stopAll: async () => {
    clearAllPendingFingerprints();
    clearAutomationRuntimeCandidateListener();
    await replaceAutomationRuntimeRules([]);
    set((current) => ({
      runtimeStates: current.rules.reduce<Record<string, AutomationRuntimeState>>((acc, rule) => {
        acc[rule.id] = deriveRuntimeState(rule.id, current.processedEntries, current.runtimeStates[rule.id], {
          status: 'stopped',
        });
        return acc;
      }, {}),
    }));
  },
}));

async function handleAutomationTaskSettled(payload: AutomationTaskSettledPayload) {
  pendingFingerprints.delete(buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint));
  clearAutomationRecoveryGuardEntry(payload.ruleId, payload.sourceFingerprint);

  const state = useAutomationStore.getState();

  // Auto-delete record if projectId is 'none' and status is 'complete'
  const rule = state.rules.find((r) => r.id === payload.ruleId);
  if (rule?.projectId === 'none' && payload.status === 'complete' && payload.historyId) {
    historyService.deleteRecording(payload.historyId).catch((err) => {
      logger.error('[Automation] Failed to auto-delete record:', err);
    });
  }

  const nextEntries = [
    ...state.processedEntries.filter((entry) => !(entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint)),
    {
      ruleId: payload.ruleId,
      filePath: payload.filePath,
      sourceFingerprint: payload.sourceFingerprint,
      size: payload.size,
      mtimeMs: payload.mtimeMs,
      status: payload.status,
      processedAt: payload.processedAt,
      historyId: payload.historyId,
      exportPath: payload.exportPath,
      errorMessage: payload.errorMessage,
    },
  ].sort((a, b) => b.processedAt - a.processedAt);

  await saveAutomationProcessedEntries(nextEntries);

  useAutomationStore.setState((current) => {
    const nextRule = current.rules.find((item) => item.id === payload.ruleId);
    const nextRuleName = nextRule?.name || rule?.name || 'Automation';
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
      })
      : upsertFailureNotification(current.notifications, {
        ruleId: payload.ruleId,
        ruleName: nextRuleName,
        filePath: payload.filePath,
        stage: payload.stage,
        message: payload.errorMessage,
        retryable: hasRetryableFailures(payload.ruleId, nextEntries),
        occurredAt: payload.processedAt,
      });

    return {
      processedEntries: nextEntries,
      notifications: nextNotifications,
      runtimeStates: {
        ...current.runtimeStates,
        [payload.ruleId]: deriveRuntimeState(payload.ruleId, nextEntries, current.runtimeStates[payload.ruleId], {
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
        }),
      },
    };
  });
}

subscribeAutomationTaskSettled(handleAutomationTaskSettled);

export async function __emitAutomationTaskSettledForTests(payload: AutomationTaskSettledPayload) {
  await emitAutomationTaskSettled(payload);
}
