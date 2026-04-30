import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { isPathInsideDirectory } from '../services/automationService';
import {
  collectAutomationRuntimeRulePaths,
  type AutomationRuntimeCandidatePayload,
  type AutomationRuntimePathCollectionResult,
  listenToAutomationRuntimeCandidates,
  toAutomationRuntimeRuleConfig,
} from '../services/automationRuntimeService';
import {
  subscribeAutomationTaskSettled,
  type AutomationTaskSettledPayload,
} from '../services/automationRuntimeBridge';
import {
  clearAutomationRecoveryGuardEntry,
  isAutomationRecoveryBlocked,
} from '../services/recoveryService';
import { historyService } from '../services/historyService';
import { logger } from '../utils/logger';
import { useBatchQueueStore } from './batchQueueStore';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import {
  persistAutomationProcessedEntries,
  validateAutomationRuleActivation,
} from './automationRepository';
import {
  applyRetryBlockedResults,
  applyRetryFailureResults,
  applyRuntimeBlockState,
  applyRuntimeFailureState,
  applyRuntimeQueuedState,
  applyTaskSettledState,
  deriveRuntimeState,
  getUniqueFilePaths,
  type AutomationSessionNotification,
  removeRuleNotifications,
} from './automationSessionState';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeBlockReason,
  AutomationRuntimeState,
} from '../types/automation';

interface HandleAutomationRuntimeCandidateOptions {
  suppressFailureNotification?: boolean;
}

type AutomationRuntimeCandidateHandleResult =
  | { status: 'queued' }
  | { status: 'blocked'; reason: AutomationRuntimeBlockReason }
  | { status: 'ignored' };

export interface AutomationRuntimeCoordinatorState {
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
  runtimeStates: Record<string, AutomationRuntimeState>;
  notifications: AutomationSessionNotification[];
}

type AutomationRuntimeCoordinatorStateUpdate =
  | Partial<AutomationRuntimeCoordinatorState>
  | ((state: AutomationRuntimeCoordinatorState) => (
    Partial<AutomationRuntimeCoordinatorState> | AutomationRuntimeCoordinatorState
  ));

interface AutomationRuntimeCoordinatorOptions {
  getState: () => AutomationRuntimeCoordinatorState;
  setState: (update: AutomationRuntimeCoordinatorStateUpdate) => void;
}

function hasAutomationItemsInFlight(ruleId: string): boolean {
  return useBatchQueueStore.getState().queueItems.some((item) => (
    item.origin === 'automation'
    && item.automationRuleId === ruleId
    && (item.status === 'pending' || item.status === 'processing')
  ));
}

export function createAutomationRuntimeCoordinator({
  getState,
  setState,
}: AutomationRuntimeCoordinatorOptions) {
  const pendingFingerprints = new Set<string>();
  let successNotificationSequence = 0;
  let automationRuntimeCandidateUnlisten: (() => void) | null = null;
  let automationTaskSettledUnlisten: (() => void) | null = null;

  function nextAutomationSuccessNotificationId(ruleId: string): string {
    successNotificationSequence += 1;
    return `automation-success-${ruleId}-${successNotificationSequence}`;
  }

  function buildPendingFingerprintKey(ruleId: string, sourceFingerprint: string): string {
    return `${ruleId}::${sourceFingerprint}`;
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

  async function handleRuntimeCandidatePayload(
    payload: AutomationRuntimeCandidatePayload,
    options?: HandleAutomationRuntimeCandidateOptions,
  ): Promise<AutomationRuntimeCandidateHandleResult> {
    const occurredAt = Date.now();
    const initialState = getState();
    const rule = initialState.rules.find((item) => item.id === payload.ruleId);
    if (!rule) {
      return { status: 'ignored' };
    }

    if (isPathInsideDirectory(payload.filePath, rule.exportConfig.directory)) {
      return { status: 'ignored' };
    }

    if (isAutomationRecoveryBlocked(payload.ruleId, payload.sourceFingerprint)) {
      setState((current) => ({
        ...applyRuntimeBlockState(current, {
          ruleId: payload.ruleId,
          filePath: payload.filePath,
          reason: 'recovery_blocked',
          occurredAt,
        }),
      }));
      return { status: 'blocked', reason: 'recovery_blocked' };
    }

    const latestState = getState();
    const latestRule = latestState.rules.find((item) => item.id === payload.ruleId);
    if (!latestRule) {
      return { status: 'ignored' };
    }

    if (latestState.processedEntries.some((entry) => (
      entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint
    ))) {
      setState((current) => ({
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
      setState((current) => ({
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
      setState((current) => {
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

    setState((current) => ({
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
    const current = getState();
    const nextState = applyRetryFailureResults(current, rule, results);
    if (nextState.processedEntries === current.processedEntries) {
      return;
    }

    await persistAutomationProcessedEntries(nextState.processedEntries);
    setState(nextState);
  }

  async function recordRetryBlockedCandidates(
    rule: AutomationRule,
    results: Array<{
      candidate: AutomationRuntimeCandidatePayload;
      reason: AutomationRuntimeBlockReason;
    }>,
  ): Promise<void> {
    const current = getState();
    const nextState = applyRetryBlockedResults(current, rule, results);
    if (nextState.processedEntries === current.processedEntries) {
      return;
    }

    await persistAutomationProcessedEntries(nextState.processedEntries);
    setState(nextState);
  }

  async function retryFailed(ruleId: string): Promise<void> {
    const state = getState();
    const rule = state.rules.find((item) => item.id === ruleId);
    if (!rule) {
      return;
    }

    const failedEntries = state.processedEntries.filter((entry) => (
      entry.ruleId === ruleId && entry.status === 'error'
    ));
    if (failedEntries.length === 0) {
      setState((current) => ({
        notifications: removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));
      return;
    }

    try {
      await validateAutomationRuleActivation(rule);
    } catch (error) {
      const lastScanAt = Date.now();
      setState((current) => ({
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
    setState((current) => ({
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
      setState((current) => ({
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
        const handled = await handleRuntimeCandidatePayload(candidate, {
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

      setState((current) => {
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
      setState((current) => ({
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

  async function handleTaskSettled(payload: AutomationTaskSettledPayload): Promise<void> {
    pendingFingerprints.delete(buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint));
    clearAutomationRecoveryGuardEntry(payload.ruleId, payload.sourceFingerprint);

    const state = getState();
    const rule = state.rules.find((item) => item.id === payload.ruleId);
    if (rule?.projectId === 'none' && payload.status === 'complete' && payload.historyId) {
      historyService.deleteRecording(payload.historyId).catch((error) => {
        logger.error('[Automation] Failed to auto-delete record:', error);
      });
    }

    const nextEntries = [
      ...state.processedEntries.filter((entry) => !(
        entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint
      )),
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

    setState((current) => applyTaskSettledState(
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

  async function ensureRuntimeCandidateListener(): Promise<void> {
    if (automationRuntimeCandidateUnlisten) {
      return;
    }

    automationRuntimeCandidateUnlisten = await listenToAutomationRuntimeCandidates((payload) => {
      void handleRuntimeCandidatePayload(payload);
    });
  }

  function clearRuntimeCandidateListener() {
    if (!automationRuntimeCandidateUnlisten) {
      return;
    }

    automationRuntimeCandidateUnlisten();
    automationRuntimeCandidateUnlisten = null;
  }

  function ensureTaskSettledListener() {
    if (automationTaskSettledUnlisten) {
      return;
    }

    automationTaskSettledUnlisten = subscribeAutomationTaskSettled((payload) => {
      void handleTaskSettled(payload);
    });
  }

  function clearTaskSettledListener() {
    automationTaskSettledUnlisten?.();
    automationTaskSettledUnlisten = null;
  }

  function clearRuntimeSessionState() {
    clearAllPendingFingerprints();
    clearRuntimeCandidateListener();
    clearTaskSettledListener();
  }

  return {
    clearAllPendingFingerprints,
    clearRulePendingFingerprints,
    clearRuntimeCandidateListener,
    clearRuntimeSessionState,
    clearTaskSettledListener,
    ensureRuntimeCandidateListener,
    ensureTaskSettledListener,
    handleRuntimeCandidatePayload,
    handleTaskSettled,
    retryFailed,
  };
}
