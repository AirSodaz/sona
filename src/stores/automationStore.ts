import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import {
  isPathInsideDirectory,
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
import type { RecoveredQueueItem } from '../types/recovery';
import { historyService } from '../services/historyService';
import { logger } from '../utils/logger';
import {
  loadAutomationRepositoryState,
  persistAutomationProcessedEntries,
  persistAutomationRepositoryState,
  persistAutomationRules,
  validateAutomationRuleActivation,
} from './automationRepository';
import {
  applyRetryBlockedResults,
  applyRetryFailureResults,
  applyRuntimeBlockState,
  applyRuntimeFailureState,
  applyRuntimeQueuedState,
  applyRuntimeReplaceResults,
  applyTaskSettledState,
  type AutomationSessionNotification,
  deriveRuntimeState,
  getUniqueFilePaths,
  rebuildRuntimeStates,
  removeRuleNotifications,
} from './automationSessionState';

const pendingFingerprints = new Set<string>();
let automationSuccessNotificationSequence = 0;
let automationRuntimeCandidateUnlisten: (() => void) | null = null;
let automationTaskSettledUnlisten: (() => void) | null = null;

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

function hasAutomationItemsInFlight(ruleId: string): boolean {
  return useBatchQueueStore.getState().queueItems.some((item) => (
    item.origin === 'automation'
    && item.automationRuleId === ruleId
    && (item.status === 'pending' || item.status === 'processing')
  ));
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
  const current = useAutomationStore.getState();
  const nextState = applyRetryFailureResults(current, rule, results);
  if (nextState.processedEntries === current.processedEntries) {
    return;
  }

  await persistAutomationProcessedEntries(nextState.processedEntries);
  useAutomationStore.setState(nextState);
}

async function recordRetryBlockedCandidates(
  rule: AutomationRule,
  results: Array<{
    candidate: AutomationRuntimeCandidatePayload;
    reason: AutomationRuntimeBlockReason;
  }>,
): Promise<void> {
  const current = useAutomationStore.getState();
  const nextState = applyRetryBlockedResults(current, rule, results);
  if (nextState.processedEntries === current.processedEntries) {
    return;
  }

  await persistAutomationProcessedEntries(nextState.processedEntries);
  useAutomationStore.setState(nextState);
}

async function validateRuleBeforeActivation(rule: AutomationRule): Promise<void> {
  await validateAutomationRuleActivation(rule);
}

async function ensureAutomationRuntimeCandidateListener() {
  if (automationRuntimeCandidateUnlisten) {
    return;
  }

  automationRuntimeCandidateUnlisten = await listenToAutomationRuntimeCandidates((payload) => {
    void handleAutomationRuntimeCandidatePayload(payload);
  });
}

function clearAutomationRuntimeCandidateListener() {
  if (!automationRuntimeCandidateUnlisten) {
    return;
  }

  automationRuntimeCandidateUnlisten();
  automationRuntimeCandidateUnlisten = null;
}

function ensureAutomationTaskSettledListener() {
  if (automationTaskSettledUnlisten) {
    return;
  }

  automationTaskSettledUnlisten = subscribeAutomationTaskSettled((payload) => {
    void handleAutomationTaskSettled(payload);
  });
}

function clearAutomationTaskSettledListener() {
  automationTaskSettledUnlisten?.();
  automationTaskSettledUnlisten = null;
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

  try {
    await validateAutomationRuleActivation(rule);
  } catch (error) {
    useAutomationStore.setState((current) => ({
      ...applyRuntimeFailureState(current, {
        ruleId,
        ruleName: rule.name,
        message: error instanceof Error ? error.message : 'Automation rule validation failed.',
        lastScanAt: Date.now(),
      }),
    }));
    throw error;
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
    await get().stopAll();
    const { rules, processedEntries } = await loadAutomationRepositoryState();

    set({
      rules,
      processedEntries,
      runtimeStates: rebuildRuntimeStates(rules, processedEntries, {}),
      notifications: [],
      isLoaded: true,
      error: null,
    });

    ensureAutomationTaskSettledListener();
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

    await persistAutomationRules(nextRules);
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

    await persistAutomationRepositoryState(nextRules, nextProcessedEntries);
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
    await persistAutomationRules(nextRules);
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

    try {
      await validateAutomationRuleActivation(rule);
    } catch (error) {
      const lastScanAt = Date.now();
      useAutomationStore.setState((current) => ({
        ...applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message: error instanceof Error ? error.message : 'Automation rule validation failed.',
          lastScanAt,
        }),
      }));
      throw error;
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
      await persistAutomationProcessedEntries(nextProcessedEntries);
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

    await persistAutomationProcessedEntries(nextProcessedEntries);
    clearAutomationRecoveryGuardEntry(item.automationRuleId, item.sourceFingerprint);
    set((current) => ({
      processedEntries: nextProcessedEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextProcessedEntries, current.runtimeStates),
    }));
  },

  stopAll: async () => {
    clearAllPendingFingerprints();
    clearAutomationRuntimeCandidateListener();
    clearAutomationTaskSettledListener();
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

  await persistAutomationProcessedEntries(nextEntries);

  useAutomationStore.setState((current) => applyTaskSettledState(
    {
      rules: current.rules,
      processedEntries: nextEntries,
      runtimeStates: current.runtimeStates,
      notifications: current.notifications,
    },
    payload,
    {
      fallbackRuleName: rule?.name,
      waveActive: hasAutomationItemsInFlight(payload.ruleId),
      nextSuccessNotificationId: () => nextAutomationSuccessNotificationId(payload.ruleId),
    },
  ));
}

export async function __emitAutomationTaskSettledForTests(payload: AutomationTaskSettledPayload) {
  await handleAutomationTaskSettled(payload);
}
