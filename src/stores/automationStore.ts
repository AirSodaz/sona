import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import {
  ensureAutomationStorage,
  isPathInsideDirectory,
  loadAutomationProcessedEntries,
  loadAutomationRules,
  saveAutomationProcessedEntries,
  saveAutomationRules,
  validateAutomationRuleForActivation,
} from '../services/automationService';
import {
  listenToAutomationRuntimeCandidates,
  replaceAutomationRuntimeRules,
  scanAutomationRuntimeRule,
  toAutomationRuntimeRuleConfig,
  type AutomationRuntimeReplaceResult,
} from '../services/automationRuntimeService';
import {
  notifyAutomationTaskSettled,
  registerAutomationTaskSettledHandler,
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
  AutomationRuntimeState,
} from '../types/automation';
import type { RecoveredQueueItem, RecoveryItemStage } from '../types/recovery';
import { historyService } from '../services/historyService';
import { logger } from '../utils/logger';

const pendingFingerprints = new Set<string>();
let automationSuccessNotificationSequence = 0;
let automationRuntimeCandidateUnlisten: (() => void) | null = null;

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
    status: overrides.status ?? existing?.status ?? 'stopped',
    lastScanAt: overrides.lastScanAt ?? existing?.lastScanAt,
    lastProcessedAt: overrides.lastProcessedAt ?? latest?.processedAt ?? existing?.lastProcessedAt,
    lastResult: overrides.lastResult ?? (latest ? (latest.status === 'complete' ? 'success' : 'error') : existing?.lastResult),
    lastResultMessage: overrides.lastResultMessage ?? latest?.errorMessage ?? existing?.lastResultMessage,
    lastProcessedFilePath: overrides.lastProcessedFilePath ?? latest?.filePath ?? existing?.lastProcessedFilePath,
    failureCount: overrides.failureCount ?? failureCount,
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

  automationRuntimeCandidateUnlisten = await listenToAutomationRuntimeCandidates(async (payload) => {
    const initialState = useAutomationStore.getState();
    const rule = initialState.rules.find((item) => item.id === payload.ruleId);
    if (!rule) {
      return;
    }

    if (isPathInsideDirectory(payload.filePath, rule.exportConfig.directory)) {
      return;
    }

    if (isAutomationRecoveryBlocked(payload.ruleId, payload.sourceFingerprint)) {
      return;
    }

    const latestState = useAutomationStore.getState();
    const latestRule = latestState.rules.find((item) => item.id === payload.ruleId);
    if (!latestRule) {
      return;
    }

    if (latestState.processedEntries.some((entry) => (
      entry.ruleId === payload.ruleId && entry.sourceFingerprint === payload.sourceFingerprint
    ))) {
      return;
    }

    const pendingKey = buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint);
    if (pendingFingerprints.has(pendingKey)) {
      return;
    }

    const isInboxOrNone = latestRule.projectId === 'inbox' || latestRule.projectId === 'none';
    const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(latestRule.projectId);
    if (!project && !isInboxOrNone) {
      useAutomationStore.setState((current) => ({
        ...applyRuntimeFailureState(current, {
          ruleId: payload.ruleId,
          ruleName: latestRule.name,
          message: 'Project not found.',
          filePath: payload.filePath,
        }),
      }));
      return;
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
  });
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
    const nextProcessedEntries = get().processedEntries.filter((entry) => !(entry.ruleId === ruleId && entry.status === 'error'));
    await saveAutomationProcessedEntries(nextProcessedEntries);
    clearRulePendingFingerprints(ruleId);
    set((current) => ({
      processedEntries: nextProcessedEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextProcessedEntries, current.runtimeStates),
      notifications: removeRuleNotifications(current.notifications, ruleId, 'failure'),
    }));
    await scanRule(ruleId);
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
  },
}));

registerAutomationTaskSettledHandler(async (payload: AutomationTaskSettledPayload) => {
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
        }),
      },
    };
  });
});

export async function __notifyAutomationTaskSettledForTests(payload: AutomationTaskSettledPayload) {
  await notifyAutomationTaskSettled(payload);
}
