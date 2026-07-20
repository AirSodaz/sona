import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import {
  replaceAutomationRuntimeRules,
  scanAutomationRuntimeRule,
  toAutomationRuntimeRuleConfig,
  type AutomationRuntimeReplaceResult,
} from '../services/automationRuntimeService';

import type {
  AutomationProcessedEntry,
  AutomationProfile,
  AutomationRule,
  AutomationRuntimeState,
} from '../types/automation';
import type { TagAutomationRunRequest } from '../services/automation/tagAutomationRun';
import type { RecoveredQueueItem } from '../types/recovery';
import { extractErrorMessage } from '../utils/errorUtils';
import {
  loadAutomationRepositoryState,
  persistAutomationProcessedEntries,
  persistAutomationProfiles,
  persistAutomationRepositoryState,
  persistAutomationRules,
  validateAutomationRuleActivation,
} from '../services/automation/automationRepository';
import {
  applyRuntimeFailureState,
  applyRuntimeReplaceResults,
  type AutomationSessionNotification,
  deriveRuntimeState,
  rebuildRuntimeStates,
  removeRuleNotifications,
  applyRetryBlockedResults,
  applyRetryFailureResults,
  applyRuntimeBlockState,
  applyRuntimeQueuedState,
  applyTaskSettledState,
  getUniqueFilePaths,
} from '../services/automation/automationSessionState';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { isPathInsideDirectory, normalizeAutomationPath } from '../services/automation/automationService';
import {
  collectAutomationRuntimeRulePaths,
  listenToAutomationRuntimeCandidates,
} from '../services/automationRuntimeService';
import {
  subscribeAutomationTaskSettled,
} from '../services/automationEventBus';
import {
  clearAutomationRecoveryGuardEntry,
  isAutomationRecoveryBlocked,
} from '../services/recoveryService';
import { historyService } from '../services/historyService';
import { applyAutomationProfile } from '../services/automation/automationConfigResolver';
import { useHistoryStore } from './historyStore';
import { useBatchQueueStore } from './batchQueueStore';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import {
  createAutomationRuntimeCoordinator,
  type AutomationRuntimeCoordinatorState,
} from '../services/automation/automationRuntimeCoordinator';

interface SaveRuleInput {
  id?: string;
  name: string;
  kind?: AutomationRule['kind'];
  priority?: number;
  profileId?: string;
  profileSource?: string;
  saveHistory?: boolean;
  tagIds?: string[];
  /** @deprecated */
  projectId?: string;
  presetId: AutomationRule['presetId'];
  watchDirectory: string;
  recursive: boolean;
  stageConfig: AutomationRule['stageConfig'];
  exportConfig: AutomationRule['exportConfig'];
  enabled?: boolean;
  actions?: AutomationRule['actions'];
  migrationNotice?: string;
}

interface SaveProfileInput extends Omit<AutomationProfile, 'id' | 'createdAt' | 'updatedAt'> {
  id?: string;
}

type AutomationProfileDependencyKind =
  | 'polishPreset'
  | 'summaryTemplate'
  | 'textReplacementSet'
  | 'hotwordSet'
  | 'polishKeywordSet'
  | 'speakerProfile';

interface AutomationState {
  profiles: AutomationProfile[];
  rules: AutomationRule[];
  processedEntries: AutomationProcessedEntry[];
  runtimeStates: Record<string, AutomationRuntimeState>;
  notifications: AutomationSessionNotification[];
  isLoaded: boolean;
  error: string | null;
  focusTagId: string | null;
  setFocusTagId: (tagId: string | null) => void;
  loadAndStart: () => Promise<void>;
  saveRule: (input: SaveRuleInput) => Promise<AutomationRule>;
  saveProfile: (input: SaveProfileInput) => Promise<AutomationProfile>;
  deleteProfile: (profileId: string) => Promise<void>;
  removeProfileDependency: (kind: AutomationProfileDependencyKind, dependencyId: string) => Promise<void>;
  deleteRule: (ruleId: string) => Promise<void>;
  toggleRuleEnabled: (ruleId: string, enabled: boolean) => Promise<void>;
  scanRuleNow: (ruleId: string) => Promise<void>;
  retryFailed: (ruleId: string) => Promise<void>;
  retryFailedFile: (ruleId: string, filePath: string) => Promise<void>;
  applyTagRuleToExisting: (ruleId: string) => Promise<number>;
  beginTagAutomationRun: (request: TagAutomationRunRequest) => Promise<boolean>;
  finishTagAutomationRun: (args: {
    ruleId: string;
    historyId: string;
    inputVersion: string;
    status: 'complete' | 'error';
    errorMessage?: string;
  }) => Promise<void>;
  dismissNotification: (notificationId: string) => void;
  retryNotification: (notificationId: string) => Promise<void>;
  markRecoveryItemDiscarded: (item: RecoveredQueueItem) => Promise<void>;
  stopAll: () => Promise<void>;
}

let processedEntryMutationQueue = Promise.resolve();

function serializeProcessedEntryMutation<T>(operation: () => Promise<T>): Promise<T> {
  const next = processedEntryMutationQueue.then(operation, operation);
  processedEntryMutationQueue = next.then(() => undefined, () => undefined);
  return next;
}

async function validateRuleBeforeActivation(rule: AutomationRule): Promise<void> {
  await validateAutomationRuleActivation(rule);
}

async function syncAutomationRuntimeRules(options?: { throwForRuleId?: string }) {
  const state = useAutomationStore.getState();
  const enabledRules = state.rules.filter((rule) => rule.enabled && (rule.kind ?? 'file') === 'file');

  await automationRuntimeCoordinator.ensureRuntimeCandidateListener();

  let results: AutomationRuntimeReplaceResult[];
  try {
    results = await replaceAutomationRuntimeRules(enabledRules.map((rule) => toAutomationRuntimeRuleConfig(rule)));
  } catch (error) {
    const message = extractErrorMessage(error);
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
    const message = extractErrorMessage(error);
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
  profiles: [],
  processedEntries: [],
  runtimeStates: {},
  notifications: [],
  isLoaded: false,
  error: null,
  focusTagId: null,

  setFocusTagId: (tagId) => set({ focusTagId: tagId }),

  loadAndStart: async () => {
    await get().stopAll();
    const { profiles, rules, processedEntries } = await loadAutomationRepositoryState();

    set({
      rules,
      profiles,
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
      kind: input.kind ?? existing?.kind ?? 'file',
      priority: input.priority ?? existing?.priority ?? 0,
      profileId: input.profileId ?? existing?.profileId,
      profileSource: input.profileSource ?? existing?.profileSource ?? 'tag_match',
      saveHistory: input.saveHistory ?? input.projectId !== 'none',
      tagIds: input.tagIds ?? (
        input.projectId && input.projectId !== 'inbox' && input.projectId !== 'none'
          ? [input.projectId]
          : []
      ),
      presetId: input.presetId,
      watchDirectory: input.watchDirectory.trim(),
      recursive: input.recursive,
      enabled: input.enabled ?? existing?.enabled ?? false,
      actions: input.actions ?? existing?.actions ?? {
        autoPolish: input.stageConfig.autoPolish,
        autoTranslate: input.stageConfig.autoTranslate,
        autoSummary: false,
      },
      stageConfig: {
        ...input.stageConfig,
      },
      exportConfig: {
        ...input.exportConfig,
        directory: input.exportConfig.directory.trim(),
      },
      createdAt: existing?.createdAt || now,
      updatedAt: now,
      migrationNotice: input.migrationNotice ?? existing?.migrationNotice,
    };

    if (nextRule.enabled && (nextRule.kind ?? 'file') === 'file') {
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

    if (nextRule.enabled && (nextRule.kind ?? 'file') === 'file') {
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

    await persistAutomationRepositoryState(get().profiles, nextRules, nextProcessedEntries);
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

  saveProfile: async (input) => {
    const now = Date.now();
    const existing = input.id ? get().profiles.find((profile) => profile.id === input.id) : undefined;
    const profile: AutomationProfile = {
      ...input,
      id: existing?.id ?? input.id ?? uuidv4(),
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    };
    const profiles = existing
      ? get().profiles.map((item) => item.id === existing.id ? profile : item)
      : [profile, ...get().profiles];
    await persistAutomationProfiles(profiles);
    set({ profiles });
    return profile;
  },

  deleteProfile: async (profileId) => {
    if (get().rules.some((rule) => rule.profileId === profileId)) {
      throw new Error('This profile is still used by an automation rule.');
    }
    const profiles = get().profiles.filter((profile) => profile.id !== profileId);
    await persistAutomationProfiles(profiles);
    set({ profiles });
  },

  removeProfileDependency: async (kind, dependencyId) => {
    const now = Date.now();
    let changed = false;
    const profiles = get().profiles.map((profile) => {
      let next = profile;
      switch (kind) {
        case 'polishPreset':
          if (profile.polishPresetId === dependencyId) {
            next = { ...profile, polishPresetId: 'general' };
          }
          break;
        case 'summaryTemplate':
          if (profile.summaryTemplateId === dependencyId) {
            next = { ...profile, summaryTemplateId: 'general' };
          }
          break;
        case 'textReplacementSet':
          if (profile.enabledTextReplacementSetIds.includes(dependencyId)) {
            next = {
              ...profile,
              enabledTextReplacementSetIds: profile.enabledTextReplacementSetIds.filter((id) => id !== dependencyId),
            };
          }
          break;
        case 'hotwordSet':
          if (profile.enabledHotwordSetIds.includes(dependencyId)) {
            next = {
              ...profile,
              enabledHotwordSetIds: profile.enabledHotwordSetIds.filter((id) => id !== dependencyId),
            };
          }
          break;
        case 'polishKeywordSet':
          if (profile.enabledPolishKeywordSetIds.includes(dependencyId)) {
            next = {
              ...profile,
              enabledPolishKeywordSetIds: profile.enabledPolishKeywordSetIds.filter((id) => id !== dependencyId),
            };
          }
          break;
        case 'speakerProfile':
          if (profile.enabledSpeakerProfileIds.includes(dependencyId)) {
            next = {
              ...profile,
              enabledSpeakerProfileIds: profile.enabledSpeakerProfileIds.filter((id) => id !== dependencyId),
            };
          }
          break;
      }
      if (next === profile) return profile;
      changed = true;
      return { ...next, updatedAt: now };
    });
    if (!changed) return;
    await persistAutomationProfiles(profiles);
    set({ profiles });
  },

  toggleRuleEnabled: async (ruleId, enabled) => {
    const state = get();
    const targetRule = state.rules.find((rule) => rule.id === ruleId);
    if (!targetRule) {
      return;
    }

    if (enabled && (targetRule.kind ?? 'file') === 'file') {
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

  retryFailedFile: async (ruleId, filePath) => {
    await automationRuntimeCoordinator.retryFailedFile(ruleId, filePath);
  },

  applyTagRuleToExisting: async (ruleId) => {
    const state = get();
    const rule = state.rules.find((item) => item.id === ruleId && item.kind === 'tag');
    if (!rule) return 0;

    const profile = rule.profileId
      ? state.profiles.find((item) => item.id === rule.profileId)
      : undefined;
    const config = applyAutomationProfile(useConfigStore.getState().config, profile);
    const matchedTagIds = new Set(rule.tagIds || []);
    const historyItems = useHistoryStore.getState().items.filter((item) => (
      item.deletedAt == null
      && (item.tagIds || []).some((tagId) => matchedTagIds.has(tagId))
    ));
    let processed = 0;

    for (const item of historyItems) {
      const loaded = await historyService.loadTranscript(item.id);
      if (!loaded?.length) continue;
      const { processTagAutomationForHistory } = await import('../services/automation/tagAutomationProcessor');
      await processTagAutomationForHistory({
        actions: rule.actions ?? { autoPolish: false, autoTranslate: false, autoSummary: false },
        config,
        historyId: item.id,
        segments: loaded,
        ruleId: rule.id,
        inputVersion: `existing:${rule.id}:${item.id}`,
        force: true,
      });
      processed += 1;
    }

    await useHistoryStore.getState().refresh();
    return processed;
  },

  beginTagAutomationRun: async (request) => serializeProcessedEntryMutation(async () => {
    const currentEntries = get().processedEntries;
    const existing = currentEntries.find((entry) => (
      entry.kind === 'tag'
      && entry.ruleId === request.ruleId
      && entry.historyId === request.historyId
      && entry.inputVersion === request.inputVersion
    ));
    if (existing?.status === 'complete' && !request.force) {
      return false;
    }

    const nextEntry: AutomationProcessedEntry = {
      id: existing?.id ?? uuidv4(),
      ruleId: request.ruleId,
      kind: 'tag',
      inputVersion: request.inputVersion,
      attempt: (existing?.attempt ?? 0) + 1,
      filePath: existing?.filePath ?? '',
      sourceFingerprint: existing?.sourceFingerprint ?? request.inputVersion,
      size: existing?.size ?? 0,
      mtimeMs: existing?.mtimeMs ?? 0,
      status: 'pending',
      processedAt: Date.now(),
      historyId: request.historyId,
      exportPath: existing?.exportPath,
      errorMessage: undefined,
    };
    const nextEntries = [
      ...currentEntries.filter((entry) => entry.id !== nextEntry.id),
      nextEntry,
    ].sort((left, right) => right.processedAt - left.processedAt);
    await persistAutomationProcessedEntries(nextEntries);
    set((current) => ({
      processedEntries: nextEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextEntries, current.runtimeStates),
    }));
    return true;
  }),

  finishTagAutomationRun: async (args) => serializeProcessedEntryMutation(async () => {
    const currentEntries = get().processedEntries;
    const nextEntries = currentEntries.map((entry) => (
      entry.kind === 'tag'
      && entry.ruleId === args.ruleId
      && entry.historyId === args.historyId
      && entry.inputVersion === args.inputVersion
        ? {
          ...entry,
          status: args.status,
          processedAt: Date.now(),
          errorMessage: args.errorMessage,
        }
        : entry
    ));
    await persistAutomationProcessedEntries(nextEntries);
    set((current) => ({
      processedEntries: nextEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextEntries, current.runtimeStates),
    }));
  }),

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
      kind: 'file',
      inputVersion: item.sourceFingerprint,
      attempt: (get().processedEntries.find((entry) => (
        entry.kind !== 'tag'
        && entry.ruleId === item.automationRuleId
        && entry.sourceFingerprint === item.sourceFingerprint
      ))?.attempt ?? 0) + 1,
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
    profiles: state.profiles,
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
  resolveEffectiveConfig,
  isPathInsideDirectory,
  normalizeAutomationPath,
  collectAutomationRuntimeRulePaths,
  listenToAutomationRuntimeCandidates,
  toAutomationRuntimeRuleConfig,
  subscribeAutomationTaskSettled,
  clearAutomationRecoveryGuardEntry,
  isAutomationRecoveryBlocked,
  historyService,
  useBatchQueueStore,
  useConfigStore,
  useProjectStore,
  persistAutomationProcessedEntries,
  validateAutomationRuleActivation,
  applyRetryBlockedResults,
  applyRetryFailureResults,
  applyRuntimeBlockState,
  applyRuntimeFailureState,
  applyRuntimeQueuedState,
  applyTaskSettledState,
  deriveRuntimeState,
  getUniqueFilePaths,
  removeRuleNotifications,
});

export async function __emitAutomationTaskSettledForTests(
  payload: Parameters<typeof automationRuntimeCoordinator.handleTaskSettled>[0],
) {
  await automationRuntimeCoordinator.handleTaskSettled(payload);
}
