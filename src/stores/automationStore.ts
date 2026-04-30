import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import {
  replaceAutomationRuntimeRules,
  scanAutomationRuntimeRule,
  toAutomationRuntimeRuleConfig,
  type AutomationRuntimeReplaceResult,
} from '../services/automationRuntimeService';
import {
  clearAutomationRecoveryGuardEntry,
} from '../services/recoveryService';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeState,
} from '../types/automation';
import type { RecoveredQueueItem } from '../types/recovery';
import {
  loadAutomationRepositoryState,
  persistAutomationProcessedEntries,
  persistAutomationRepositoryState,
  persistAutomationRules,
  validateAutomationRuleActivation,
} from './automationRepository';
import {
  applyRuntimeFailureState,
  applyRuntimeReplaceResults,
  type AutomationSessionNotification,
  deriveRuntimeState,
  rebuildRuntimeStates,
  removeRuleNotifications,
} from './automationSessionState';
import {
  createAutomationRuntimeCoordinator,
  type AutomationRuntimeCoordinatorState,
} from './automationRuntimeCoordinator';

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

async function validateRuleBeforeActivation(rule: AutomationRule): Promise<void> {
  await validateAutomationRuleActivation(rule);
}

async function syncAutomationRuntimeRules(options?: { throwForRuleId?: string }) {
  const state = useAutomationStore.getState();
  const enabledRules = state.rules.filter((rule) => rule.enabled);

  await automationRuntimeCoordinator.ensureRuntimeCandidateListener();

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

  await automationRuntimeCoordinator.ensureRuntimeCandidateListener();

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

    automationRuntimeCoordinator.ensureTaskSettledListener();
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
      automationRuntimeCoordinator.clearRulePendingFingerprints(nextRule.id);
      await syncAutomationRuntimeRules();
    }

    return nextRule;
  },

  deleteRule: async (ruleId) => {
    const nextRules = get().rules.filter((rule) => rule.id !== ruleId);
    const nextProcessedEntries = get().processedEntries.filter((entry) => entry.ruleId !== ruleId);

    await persistAutomationRepositoryState(nextRules, nextProcessedEntries);
    automationRuntimeCoordinator.clearRulePendingFingerprints(ruleId);

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

    automationRuntimeCoordinator.clearRulePendingFingerprints(ruleId);
    await syncAutomationRuntimeRules();
  },

  scanRuleNow: async (ruleId) => {
    await scanRule(ruleId);
  },

  retryFailed: async (ruleId) => {
    await automationRuntimeCoordinator.retryFailed(ruleId);
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
    automationRuntimeCoordinator.clearRuntimeSessionState();
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

function getAutomationRuntimeCoordinatorState(state: AutomationState): AutomationRuntimeCoordinatorState {
  return {
    rules: state.rules,
    processedEntries: state.processedEntries,
    runtimeStates: state.runtimeStates,
    notifications: state.notifications,
  };
}

const automationRuntimeCoordinator = createAutomationRuntimeCoordinator({
  getState: () => getAutomationRuntimeCoordinatorState(useAutomationStore.getState()),
  setState: (update) => {
    useAutomationStore.setState((current) => {
      const currentState = getAutomationRuntimeCoordinatorState(current);
      return typeof update === 'function' ? update(currentState) : update;
    });
  },
});

export async function __emitAutomationTaskSettledForTests(
  payload: Parameters<typeof automationRuntimeCoordinator.handleTaskSettled>[0],
) {
  await automationRuntimeCoordinator.handleTaskSettled(payload);
}
