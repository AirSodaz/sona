import type {
  AutomationProcessedEntry,
  AutomationProfile,
  AutomationResolutionSnapshot,
  AutomationRule,
  AutomationRuntimeBlockReason,
  AutomationRuntimeState,
  AutomationStageConfig,
} from '../../types/automation';
import type { AppConfig } from '../../types/config';
import type {
  AutomationRuntimeCandidatePayload,
  AutomationRuntimePathCollectionResult,
} from '../automationRuntimeService';
import type {
  AutomationTaskSettledPayload,
} from '../automationEventBus';
import type {
  AutomationSessionNotification,
} from './automationSessionState';
import { resolveEffectiveConfig } from '../effectiveConfigService';
import { resolveAutomationQueueSnapshot } from './automationConfigResolver';
import { isPathInsideDirectory, normalizeAutomationPath } from '../automation/automationService';
import {
  collectAutomationRuntimeRulePaths,
  listenToAutomationRuntimeCandidates,
  toAutomationRuntimeRuleConfig,
} from '../automationRuntimeService';
import {
  subscribeAutomationTaskSettled,
} from '../automationEventBus';
import {
  clearAutomationRecoveryGuardEntry,
  isAutomationRecoveryBlocked,
} from '../recoveryService';
import { historyService } from '../historyService';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
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
  removeRuleNotifications,
} from './automationSessionState';

interface HandleAutomationRuntimeCandidateOptions {
  suppressFailureNotification?: boolean;
}

type AutomationRuntimeCandidateHandleResult =
  | { status: 'queued' }
  | { status: 'blocked'; reason: AutomationRuntimeBlockReason }
  | { status: 'ignored' };

export interface AutomationRuntimeCoordinatorState {
  profiles: AutomationProfile[];
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
  runtimeStates: Record<string, AutomationRuntimeState>;
  notifications: AutomationSessionNotification[];
}

export type AutomationRuntimeCoordinatorStateUpdate =
  | Partial<AutomationRuntimeCoordinatorState>
  | ((state: AutomationRuntimeCoordinatorState) => (
    Partial<AutomationRuntimeCoordinatorState> | AutomationRuntimeCoordinatorState
  ));

export interface AutomationRuntimeCoordinatorPorts {
  getState: () => AutomationRuntimeCoordinatorState;
  setState: (update: AutomationRuntimeCoordinatorStateUpdate) => void;
  resolveEffectiveConfig: typeof resolveEffectiveConfig;
  isPathInsideDirectory: typeof isPathInsideDirectory;
  normalizeAutomationPath: typeof normalizeAutomationPath;
  collectAutomationRuntimeRulePaths: typeof collectAutomationRuntimeRulePaths;
  listenToAutomationRuntimeCandidates: typeof listenToAutomationRuntimeCandidates;
  toAutomationRuntimeRuleConfig: typeof toAutomationRuntimeRuleConfig;
  subscribeAutomationTaskSettled: typeof subscribeAutomationTaskSettled;
  clearAutomationRecoveryGuardEntry: typeof clearAutomationRecoveryGuardEntry;
  isAutomationRecoveryBlocked: typeof isAutomationRecoveryBlocked;
  historyService: typeof historyService;
  useBatchQueueStore: typeof useBatchQueueStore;
  useConfigStore: typeof useConfigStore;
  useProjectStore: typeof useProjectStore;
  persistAutomationProcessedEntries: typeof persistAutomationProcessedEntries;
  validateAutomationRuleActivation: typeof validateAutomationRuleActivation;
  applyRetryBlockedResults: typeof applyRetryBlockedResults;
  applyRetryFailureResults: typeof applyRetryFailureResults;
  applyRuntimeBlockState: typeof applyRuntimeBlockState;
  applyRuntimeFailureState: typeof applyRuntimeFailureState;
  applyRuntimeQueuedState: typeof applyRuntimeQueuedState;
  applyTaskSettledState: typeof applyTaskSettledState;
  deriveRuntimeState: typeof deriveRuntimeState;
  getUniqueFilePaths: typeof getUniqueFilePaths;
  removeRuleNotifications: typeof removeRuleNotifications;
}

export class AutomationRuntimeCoordinator {
  private pendingFingerprints = new Set<string>();
  private successNotificationSequence = 0;
  private automationRuntimeCandidateUnlisten: (() => void) | null = null;
  private automationTaskSettledUnlisten: (() => void) | null = null;

  constructor(private readonly ports: AutomationRuntimeCoordinatorPorts) {}

  private hasAutomationItemsInFlight = (ruleId: string): boolean => {
    return this.ports.useBatchQueueStore.getState().queueItems.some((item) => (
      item.origin === 'automation'
      && item.automationRuleId === ruleId
      && (item.status === 'pending' || item.status === 'processing')
    ));
  }

  private nextAutomationSuccessNotificationId = (ruleId: string): string => {
    this.successNotificationSequence += 1;
    return `automation-success-${ruleId}-${this.successNotificationSequence}`;
  }

  private buildPendingFingerprintKey = (ruleId: string, sourceFingerprint: string): string => {
    return `${ruleId}::${sourceFingerprint}`;
  }

  clearRulePendingFingerprints = (ruleId: string): void => {
    for (const key of this.pendingFingerprints.values()) {
      if (key.startsWith(`${ruleId}::`)) {
        this.pendingFingerprints.delete(key);
      }
    }
  }

  clearAllPendingFingerprints = (): void => {
    this.pendingFingerprints.clear();
  }

  handleRuntimeCandidatePayload = async (
    payload: AutomationRuntimeCandidatePayload,
    options?: HandleAutomationRuntimeCandidateOptions,
  ): Promise<AutomationRuntimeCandidateHandleResult> => {
    const occurredAt = Date.now();
    const initialState = this.ports.getState();
    const rule = initialState.rules.find((item) => item.id === payload.ruleId);
    if (!rule) {
      return { status: 'ignored' };
    }

    if (this.ports.isPathInsideDirectory(payload.filePath, rule.exportConfig.directory)) {
      return { status: 'ignored' };
    }

    if (this.ports.isAutomationRecoveryBlocked(payload.ruleId, payload.sourceFingerprint)) {
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeBlockState(current, {
          ruleId: payload.ruleId,
          filePath: payload.filePath,
          reason: 'recovery_blocked',
          occurredAt,
        }),
      }));
      return { status: 'blocked', reason: 'recovery_blocked' };
    }

    const latestState = this.ports.getState();
    const latestRule = latestState.rules.find((item) => item.id === payload.ruleId);
    if (!latestRule) {
      return { status: 'ignored' };
    }

    if (latestState.processedEntries.some((entry) => (
      entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint
    ))) {
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeBlockState(current, {
          ruleId: payload.ruleId,
          filePath: payload.filePath,
          reason: 'already_processed',
          occurredAt,
        }),
      }));
      return { status: 'blocked', reason: 'already_processed' };
    }

    const pendingKey = this.buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint);
    if (this.pendingFingerprints.has(pendingKey)) {
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeBlockState(current, {
          ruleId: payload.ruleId,
          filePath: payload.filePath,
          reason: 'already_pending',
          occurredAt,
        }),
      }));
      return { status: 'blocked', reason: 'already_pending' };
    }

    const tagIds = latestRule.tagIds ?? (
      latestRule.projectId && latestRule.projectId !== 'inbox' && latestRule.projectId !== 'none'
        ? [latestRule.projectId]
        : []
    );
    const projectStore = this.ports.useProjectStore.getState();
    const tags = tagIds
      .map((tagId) => projectStore.getProjectById(tagId))
      .filter((tag): tag is NonNullable<typeof tag> => !!tag);
    if (tags.length !== tagIds.length) {
      this.ports.setState((current) => {
        if (options?.suppressFailureNotification) {
          const runtimeStatesWithError = {
            ...current.runtimeStates,
            [payload.ruleId]: this.ports.deriveRuntimeState(
              payload.ruleId,
              current.processedEntries,
              current.runtimeStates[payload.ruleId],
              {
                status: 'error',
                lastResult: 'error',
                lastResultMessage: 'Tag not found.',
                lastProcessedFilePath: payload.filePath,
              },
            ),
          };

          return {
            runtimeStates: this.ports.applyRuntimeBlockState(
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

        const nextFailureState = this.ports.applyRuntimeFailureState(current, {
          ruleId: payload.ruleId,
          ruleName: latestRule.name,
          message: 'Tag not found.',
          filePath: payload.filePath,
        });

        return {
          ...nextFailureState,
          ...this.ports.applyRuntimeBlockState(
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

    this.pendingFingerprints.add(pendingKey);
    let effectiveConfig: AppConfig;
    let resolvedStageConfig: AutomationStageConfig;
    let automationResolutionSnapshot: AutomationResolutionSnapshot;
    try {
      const snapshot = resolveAutomationQueueSnapshot({
        globalConfig: this.ports.useConfigStore.getState().config,
        profiles: latestState.profiles,
        rules: latestState.rules,
        fileRule: latestRule,
        tagIds,
      });
      effectiveConfig = snapshot.config;
      resolvedStageConfig = snapshot.stageConfig;
      automationResolutionSnapshot = snapshot.resolution;
    } catch (error) {
      this.pendingFingerprints.delete(pendingKey);
      throw error;
    }

    try {
      this.ports.useBatchQueueStore.getState().addFiles([payload.filePath], {
        origin: 'automation',
        automationRuleId: latestRule.id,
        automationRuleName: latestRule.name,
        resolvedConfigSnapshot: effectiveConfig,
        exportConfig: latestRule.stageConfig.exportEnabled ? latestRule.exportConfig : null,
        stageConfig: resolvedStageConfig,
        automationResolutionSnapshot,
        sourceFingerprint: payload.sourceFingerprint,
        tagIds,
        fileStat: {
          size: payload.size,
          mtimeMs: payload.mtimeMs,
        },
        exportFileNamePrefix: latestRule.exportConfig.prefix || '',
      });
    } catch (error) {
      this.pendingFingerprints.delete(pendingKey);
      throw error;
    }

    this.ports.setState((current) => ({
      ...this.ports.applyRuntimeQueuedState(current, {
        ruleId: payload.ruleId,
        occurredAt,
      }),
    }));

    return { status: 'queued' };
  }

  private recordRetryFailures = async (
    rule: AutomationRule,
    results: AutomationRuntimePathCollectionResult[],
  ): Promise<void> => {
    const current = this.ports.getState();
    const nextState = this.ports.applyRetryFailureResults(current, rule, results);
    if (nextState.processedEntries === current.processedEntries) {
      return;
    }

    await this.ports.persistAutomationProcessedEntries(nextState.processedEntries);
    this.ports.setState(nextState);
  }

  private recordRetryBlockedCandidates = async (
    rule: AutomationRule,
    results: Array<{
      candidate: AutomationRuntimeCandidatePayload;
      reason: AutomationRuntimeBlockReason;
    }>,
  ): Promise<void> => {
    const current = this.ports.getState();
    const nextState = this.ports.applyRetryBlockedResults(current, rule, results);
    if (nextState.processedEntries === current.processedEntries) {
      return;
    }

    await this.ports.persistAutomationProcessedEntries(nextState.processedEntries);
    this.ports.setState(nextState);
  }

  retryFailed = async (ruleId: string): Promise<void> => {
    const state = this.ports.getState();
    const rule = state.rules.find((item) => item.id === ruleId);
    if (!rule) {
      return;
    }

    const failedEntries = state.processedEntries.filter((entry) => (
      entry.ruleId === ruleId && entry.status === 'error'
    ));
    if (failedEntries.length === 0) {
      this.ports.setState((current) => ({
        notifications: this.ports.removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));
      return;
    }

    try {
      await this.ports.validateAutomationRuleActivation(rule);
    } catch (error) {
      const lastScanAt = Date.now();
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message: error instanceof Error ? error.message : 'Automation rule validation failed.',
          lastScanAt,
        }),
      }));
      throw error;
    }

    const scanStartedAt = Date.now();
    this.ports.setState((current) => ({
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: this.ports.deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'scanning',
          lastScanAt: scanStartedAt,
        }),
      },
    }));

    const filePaths = this.ports.getUniqueFilePaths(failedEntries.map((entry) => entry.filePath));

    try {
      const results = await this.ports.collectAutomationRuntimeRulePaths(
        this.ports.toAutomationRuntimeRuleConfig(rule),
        filePaths,
      );

      const nextProcessedEntries = state.processedEntries.filter((entry) => !(
        entry.ruleId === ruleId && entry.status === 'error'
      ));
      await this.ports.persistAutomationProcessedEntries(nextProcessedEntries);
      this.ports.setState((current) => ({
        processedEntries: nextProcessedEntries,
        runtimeStates: {
          ...current.runtimeStates,
          [ruleId]: this.ports.deriveRuntimeState(ruleId, nextProcessedEntries, current.runtimeStates[ruleId], {
            status: 'scanning',
            lastScanAt: scanStartedAt,
          }),
        },
        notifications: this.ports.removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));

      const failureResults = results.filter((result) => result.outcome !== 'candidate');
      const candidateResults = results.filter((result) => (
        result.outcome === 'candidate' && result.candidate
      ));
      const blockedCandidateFailures: Array<{
        candidate: AutomationRuntimeCandidatePayload;
        reason: AutomationRuntimeBlockReason;
      }> = [];

      const handledCandidateResults = await Promise.all(candidateResults.map(async (result) => {
        const candidate = result.candidate!;
        const handled = await this.handleRuntimeCandidatePayload(candidate, {
          suppressFailureNotification: true,
        });
        return { candidate, handled };
      }));

      for (const { candidate, handled } of handledCandidateResults) {
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

      await this.recordRetryFailures(rule, failureResults);
      await this.recordRetryBlockedCandidates(rule, blockedCandidateFailures);

      this.ports.setState((current) => {
        const runtime = current.runtimeStates[ruleId];
        if (runtime?.status !== 'scanning') {
          return {};
        }

        return {
          runtimeStates: {
            ...current.runtimeStates,
            [ruleId]: this.ports.deriveRuntimeState(ruleId, current.processedEntries, runtime, {
              status: rule.enabled ? 'watching' : 'stopped',
              lastScanAt: Date.now(),
            }),
          },
        };
      });
    } catch (error) {
      const message = extractErrorMessage(error);
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message,
          lastScanAt: Date.now(),
        }),
      }));
      throw error;
    }
  }

  retryFailedFile = async (ruleId: string, filePath: string): Promise<void> => {
    const state = this.ports.getState();
    const rule = state.rules.find((item) => item.id === ruleId);
    if (!rule) {
      throw new Error('Automation rule not found.');
    }

    try {
      await this.ports.validateAutomationRuleActivation(rule);
    } catch (error) {
      const lastScanAt = Date.now();
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message: error instanceof Error ? error.message : 'Automation rule validation failed.',
          filePath,
          lastScanAt,
        }),
      }));
      throw error;
    }

    const scanStartedAt = Date.now();
    this.ports.setState((current) => ({
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: this.ports.deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'scanning',
          lastScanAt: scanStartedAt,
        }),
      },
    }));

    const normalizedTargetPath = this.ports.normalizeAutomationPath(filePath);

    try {
      const results = await this.ports.collectAutomationRuntimeRulePaths(
        this.ports.toAutomationRuntimeRuleConfig(rule),
        [filePath],
      );

      const nextProcessedEntries = this.ports.getState().processedEntries.filter((entry) => !(
        entry.ruleId === ruleId
        && entry.status === 'error'
        && this.ports.normalizeAutomationPath(entry.filePath) === normalizedTargetPath
      ));
      await this.ports.persistAutomationProcessedEntries(nextProcessedEntries);
      this.ports.setState((current) => ({
        processedEntries: nextProcessedEntries,
        runtimeStates: {
          ...current.runtimeStates,
          [ruleId]: this.ports.deriveRuntimeState(ruleId, nextProcessedEntries, current.runtimeStates[ruleId], {
            status: 'scanning',
            lastScanAt: scanStartedAt,
          }),
        },
        notifications: this.ports.removeRuleNotifications(current.notifications, ruleId, 'failure'),
      }));

      const failureResults = results.filter((result) => result.outcome !== 'candidate');
      const candidateResults = results.filter((result) => (
        result.outcome === 'candidate' && result.candidate
      ));
      const blockedCandidateFailures: Array<{
        candidate: AutomationRuntimeCandidatePayload;
        reason: AutomationRuntimeBlockReason;
      }> = [];

      const handledCandidateResults = await Promise.all(candidateResults.map(async (result) => {
        const candidate = result.candidate!;
        const handled = await this.handleRuntimeCandidatePayload(candidate, {
          suppressFailureNotification: true,
        });
        return { candidate, handled };
      }));

      for (const { candidate, handled } of handledCandidateResults) {
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

      await this.recordRetryFailures(rule, failureResults);
      await this.recordRetryBlockedCandidates(rule, blockedCandidateFailures);

      this.ports.setState((current) => {
        const runtime = current.runtimeStates[ruleId];
        if (runtime?.status !== 'scanning') {
          return {};
        }

        return {
          runtimeStates: {
            ...current.runtimeStates,
            [ruleId]: this.ports.deriveRuntimeState(ruleId, current.processedEntries, runtime, {
              status: rule.enabled ? 'watching' : 'stopped',
              lastScanAt: Date.now(),
            }),
          },
        };
      });
    } catch (error) {
      const message = extractErrorMessage(error);
      this.ports.setState((current) => ({
        ...this.ports.applyRuntimeFailureState(current, {
          ruleId,
          ruleName: rule.name,
          message,
          filePath,
          lastScanAt: Date.now(),
        }),
      }));
      throw error;
    }
  }

  handleTaskSettled = async (payload: AutomationTaskSettledPayload): Promise<void> => {
    this.pendingFingerprints.delete(this.buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint));
    this.ports.clearAutomationRecoveryGuardEntry(payload.ruleId, payload.sourceFingerprint);

    const state = this.ports.getState();
    const rule = state.rules.find((item) => item.id === payload.ruleId);
    if (rule?.saveHistory === false && payload.status === 'complete' && payload.historyId) {
      this.ports.historyService.deleteRecording(payload.historyId)
        .then(() => this.ports.historyService.purgeRecordings([payload.historyId as string]))
        .catch((error) => {
        logger.error('[Automation] Failed to auto-delete record:', error);
        });
    }

    const nextEntries = [
      ...state.processedEntries.filter((entry) => !(
        entry.kind !== 'tag'
        && entry.ruleId === payload.ruleId
        && entry.sourceFingerprint === payload.sourceFingerprint
      )),
      {
        ruleId: payload.ruleId,
        kind: 'file' as const,
        inputVersion: payload.sourceFingerprint,
        attempt: (state.processedEntries.find((entry) => (
          entry.kind !== 'tag'
          && entry.ruleId === payload.ruleId
          && entry.sourceFingerprint === payload.sourceFingerprint
        ))?.attempt ?? 0) + 1,
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

    await this.ports.persistAutomationProcessedEntries(nextEntries);

    this.ports.setState((current) => this.ports.applyTaskSettledState(
      {
        rules: current.rules,
        processedEntries: nextEntries,
        runtimeStates: current.runtimeStates,
        notifications: current.notifications,
      },
      payload,
      {
        fallbackRuleName: rule?.name,
        waveActive: this.hasAutomationItemsInFlight(payload.ruleId),
        nextSuccessNotificationId: () => this.nextAutomationSuccessNotificationId(payload.ruleId),
      },
    ));
  }

  ensureRuntimeCandidateListener = async (): Promise<void> => {
    if (this.automationRuntimeCandidateUnlisten) {
      return;
    }

    this.automationRuntimeCandidateUnlisten = await this.ports.listenToAutomationRuntimeCandidates((payload) => {
      void this.handleRuntimeCandidatePayload(payload);
    });
  }

  clearRuntimeCandidateListener = (): void => {
    if (!this.automationRuntimeCandidateUnlisten) {
      return;
    }

    this.automationRuntimeCandidateUnlisten();
    this.automationRuntimeCandidateUnlisten = null;
  }

  ensureTaskSettledListener = (): void => {
    if (this.automationTaskSettledUnlisten) {
      return;
    }

    this.automationTaskSettledUnlisten = this.ports.subscribeAutomationTaskSettled((payload) => {
      void this.handleTaskSettled(payload);
    });
  }

  clearTaskSettledListener = (): void => {
    this.automationTaskSettledUnlisten?.();
    this.automationTaskSettledUnlisten = null;
  }

  clearRuntimeSessionState = (): void => {
    this.clearAllPendingFingerprints();
    this.clearRuntimeCandidateListener();
    this.clearTaskSettledListener();
  }
}

export function createAutomationRuntimeCoordinator(ports: AutomationRuntimeCoordinatorPorts): AutomationRuntimeCoordinator {
  return new AutomationRuntimeCoordinator(ports);
}
