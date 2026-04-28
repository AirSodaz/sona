import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import {
  ensureAutomationStorage,
  isPathInsideDirectory,
  listFilesRecursively,
  loadAutomationProcessedEntries,
  loadAutomationRules,
  normalizeAutomationPath,
  saveAutomationProcessedEntries,
  saveAutomationRules,
  validateAutomationRuleForActivation,
  waitForStableAutomationFile,
  watchAutomationDirectory,
} from '../services/automationService';
import {
  notifyAutomationTaskSettled,
  registerAutomationTaskSettledHandler,
  type AutomationTaskSettledPayload,
} from '../services/automationRuntimeBridge';
import { useBatchQueueStore } from './batchQueueStore';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeState,
} from '../types/automation';
import { historyService } from '../services/historyService';
import { logger } from '../utils/logger';

const FILE_STABLE_WINDOW_MS = 5000;
const CANDIDATE_DEBOUNCE_MS = 250;

const ruleWatchers = new Map<string, () => void>();
const candidateTimers = new Map<string, ReturnType<typeof setTimeout>>();
const pendingFingerprints = new Set<string>();

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
  isLoaded: boolean;
  error: string | null;
  loadAndStart: () => Promise<void>;
  saveRule: (input: SaveRuleInput) => Promise<AutomationRule>;
  deleteRule: (ruleId: string) => Promise<void>;
  toggleRuleEnabled: (ruleId: string, enabled: boolean) => Promise<void>;
  scanRuleNow: (ruleId: string) => Promise<void>;
  retryFailed: (ruleId: string) => Promise<void>;
  stopAll: () => Promise<void>;
}

function deriveRuntimeState(
  ruleId: string,
  entries: AutomationProcessedEntry[],
  existing: AutomationRuntimeState | undefined,
  overrides: Partial<AutomationRuntimeState> = {},
): AutomationRuntimeState {
  const ruleEntries = entries
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

function clearRuleCandidateTimers(ruleId: string) {
  for (const [key, timer] of candidateTimers.entries()) {
    if (key.startsWith(`${ruleId}::`)) {
      clearTimeout(timer);
      candidateTimers.delete(key);
    }
  }
}

function clearRulePendingFingerprints(ruleId: string) {
  for (const key of pendingFingerprints.values()) {
    if (key.startsWith(`${ruleId}::`)) {
      pendingFingerprints.delete(key);
    }
  }
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

async function stopRuleRuntime(ruleId: string) {
  const unwatch = ruleWatchers.get(ruleId);
  if (unwatch) {
    try {
      unwatch();
    } catch (error) {
      logger.warn('[Automation] Failed to unwatch rule:', ruleId, error);
    }
    ruleWatchers.delete(ruleId);
  }

  clearRuleCandidateTimers(ruleId);
  clearRulePendingFingerprints(ruleId);
}

async function scheduleCandidate(ruleId: string, filePath: string) {
  const normalizedPath = normalizeAutomationPath(filePath);
  const timerKey = `${ruleId}::${normalizedPath}`;
  const existingTimer = candidateTimers.get(timerKey);
  if (existingTimer) {
    clearTimeout(existingTimer);
  }

  candidateTimers.set(timerKey, setTimeout(async () => {
    candidateTimers.delete(timerKey);

    const initialState = useAutomationStore.getState();
    const rule = initialState.rules.find((item) => item.id === ruleId);
    if (!rule) {
      return;
    }

    if (isPathInsideDirectory(filePath, rule.exportConfig.directory)) {
      return;
    }

    const snapshot = await waitForStableAutomationFile(filePath, FILE_STABLE_WINDOW_MS);
    if (!snapshot) {
      return;
    }

    const latestState = useAutomationStore.getState();
    const latestRule = latestState.rules.find((item) => item.id === ruleId);
    if (!latestRule) {
      return;
    }

    if (latestState.processedEntries.some((entry) => entry.ruleId === ruleId && entry.sourceFingerprint === snapshot.sourceFingerprint)) {
      return;
    }

    const pendingKey = buildPendingFingerprintKey(ruleId, snapshot.sourceFingerprint);
    if (pendingFingerprints.has(pendingKey)) {
      return;
    }

    const isInboxOrNone = latestRule.projectId === 'inbox' || latestRule.projectId === 'none';
    const project = isInboxOrNone ? null : useProjectStore.getState().getProjectById(latestRule.projectId);
    if (!project && !isInboxOrNone) {
      useAutomationStore.setState((current) => ({
        runtimeStates: {
          ...current.runtimeStates,
          [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
            status: 'error',
            lastResult: 'error',
            lastResultMessage: 'Project not found.',
          }),
        },
      }));
      return;
    }

    const effectiveConfig = {
      ...resolveEffectiveConfig(useConfigStore.getState().config, project),
      translationLanguage: latestRule.stageConfig.translationLanguage || 'en',
      polishPresetId: latestRule.stageConfig.polishPresetId || 'general',
    };
    pendingFingerprints.add(pendingKey);

    useBatchQueueStore.getState().addFiles([snapshot.filePath], {
      origin: 'automation',
      automationRuleId: latestRule.id,
      automationRuleName: latestRule.name,
      resolvedConfigSnapshot: effectiveConfig,
      exportConfig: latestRule.stageConfig.exportEnabled ? latestRule.exportConfig : null,
      stageConfig: latestRule.stageConfig,
      sourceFingerprint: snapshot.sourceFingerprint,
      projectId: isInboxOrNone ? null : latestRule.projectId,
      fileStat: {
        size: snapshot.size,
        mtimeMs: snapshot.mtimeMs,
      },
      exportFileNamePrefix: latestRule.exportConfig.prefix || project?.defaults.exportFileNamePrefix || '',
    });
  }, CANDIDATE_DEBOUNCE_MS));
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
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'error',
          lastScanAt: Date.now(),
          lastResult: 'error',
          lastResultMessage: validation.message,
        }),
      },
    }));
    throw new Error(validation.message || 'Automation rule validation failed.');
  }

  useAutomationStore.setState((current) => ({
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        status: 'scanning',
        lastScanAt: Date.now(),
      }),
    },
  }));

  const files = await listFilesRecursively(rule.watchDirectory, rule.recursive);
  for (const filePath of files) {
    if (isPathInsideDirectory(filePath, rule.exportConfig.directory)) {
      continue;
    }
    await scheduleCandidate(rule.id, filePath);
  }

  useAutomationStore.setState((current) => ({
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        status: rule.enabled ? 'watching' : 'stopped',
        lastScanAt: Date.now(),
      }),
    },
  }));
}

async function startRuleRuntime(ruleId: string) {
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
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'error',
          lastResult: 'error',
          lastResultMessage: validation.message,
        }),
      },
    }));
    throw new Error(validation.message || 'Automation rule validation failed.');
  }

  await stopRuleRuntime(ruleId);

  const unwatch = await watchAutomationDirectory(rule.watchDirectory, rule.recursive, (paths) => {
    paths.forEach((candidatePath) => {
      void scheduleCandidate(rule.id, candidatePath);
    });
  });
  ruleWatchers.set(rule.id, unwatch);

  useAutomationStore.setState((current) => ({
    runtimeStates: {
      ...current.runtimeStates,
      [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
        status: 'watching',
        lastResultMessage: undefined,
      }),
    },
  }));

  await scanRule(ruleId);
}

export const useAutomationStore = create<AutomationState>((set, get) => ({
  rules: [],
  processedEntries: [],
  runtimeStates: {},
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
      isLoaded: true,
      error: null,
    });

    for (const rule of rules) {
      if (!rule.enabled) {
        continue;
      }

      try {
        await startRuleRuntime(rule.id);
      } catch (error) {
        logger.error('[Automation] Failed to restore enabled rule:', rule.id, error);
      }
    }
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
      await startRuleRuntime(nextRule.id);
    } else if (existing?.enabled) {
      await stopRuleRuntime(nextRule.id);
      set((current) => ({
        runtimeStates: {
          ...current.runtimeStates,
          [nextRule.id]: deriveRuntimeState(nextRule.id, current.processedEntries, current.runtimeStates[nextRule.id], {
            status: 'stopped',
          }),
        },
      }));
    }

    return nextRule;
  },

  deleteRule: async (ruleId) => {
    const nextRules = get().rules.filter((rule) => rule.id !== ruleId);
    const nextProcessedEntries = get().processedEntries.filter((entry) => entry.ruleId !== ruleId);

    await stopRuleRuntime(ruleId);
    await Promise.all([
      saveAutomationRules(nextRules),
      saveAutomationProcessedEntries(nextProcessedEntries),
    ]);

    set((current) => {
      const nextRuntimeStates = { ...current.runtimeStates };
      delete nextRuntimeStates[ruleId];
      return {
        rules: nextRules,
        processedEntries: nextProcessedEntries,
        runtimeStates: nextRuntimeStates,
      };
    });
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
      await startRuleRuntime(ruleId);
      return;
    }

    await stopRuleRuntime(ruleId);
    set((current) => ({
      runtimeStates: {
        ...current.runtimeStates,
        [ruleId]: deriveRuntimeState(ruleId, current.processedEntries, current.runtimeStates[ruleId], {
          status: 'stopped',
        }),
      },
    }));
  },

  scanRuleNow: async (ruleId) => {
    await scanRule(ruleId);
  },

  retryFailed: async (ruleId) => {
    const nextProcessedEntries = get().processedEntries.filter((entry) => !(entry.ruleId === ruleId && entry.status === 'error'));
    await saveAutomationProcessedEntries(nextProcessedEntries);
    set((current) => ({
      processedEntries: nextProcessedEntries,
      runtimeStates: rebuildRuntimeStates(current.rules, nextProcessedEntries, current.runtimeStates),
    }));
    await scanRule(ruleId);
  },

  stopAll: async () => {
    const ruleIds = get().rules.map((rule) => rule.id);
    await Promise.all(ruleIds.map((ruleId) => stopRuleRuntime(ruleId)));
  },
}));

registerAutomationTaskSettledHandler(async (payload: AutomationTaskSettledPayload) => {
  pendingFingerprints.delete(buildPendingFingerprintKey(payload.ruleId, payload.sourceFingerprint));

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

  useAutomationStore.setState((current) => ({
    processedEntries: nextEntries,
    runtimeStates: {
      ...current.runtimeStates,
      [payload.ruleId]: deriveRuntimeState(payload.ruleId, nextEntries, current.runtimeStates[payload.ruleId], {
        status: current.rules.find((rule) => rule.id === payload.ruleId)?.enabled ? 'watching' : 'stopped',
        lastProcessedAt: payload.processedAt,
        lastProcessedFilePath: payload.filePath,
        lastResult: payload.status === 'complete' ? 'success' : 'error',
        lastResultMessage: payload.errorMessage,
      }),
    },
  }));
});

export async function __notifyAutomationTaskSettledForTests(payload: AutomationTaskSettledPayload) {
  await notifyAutomationTaskSettled(payload);
}
