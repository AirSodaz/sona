import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { speakerService } from '../services/speakerService';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useModelDownloadStore } from '../stores/modelDownloadStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useSpeakerNotificationStore } from '../stores/speakerNotificationStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import type { NotificationEntry, NotificationGroup } from '../types/notification';
import type { SettingsTab } from '../types/settings';
import { isTaskLedgerActiveStatus } from '../types/taskLedger';
import { getResumeOnboardingStep, shouldShowOnboardingReminder } from '../utils/onboarding';
import { adaptModelDownloadsEntries } from './notifications/modelDownloadAdapter';
import { adaptModelDownloadEntry, adaptOnboardingEntry } from './notifications/onboardingAdapter';
import { adaptSpeakerDegradedEntry } from './notifications/speakerDiarizationAdapter';
import { adaptTaskLedgerEntries } from './notifications/taskLedgerAdapter';
import { adaptUpdateEntry } from './notifications/updateAdapter';
import { useAppUpdater } from './useAppUpdater';
import { cleanupLedgerTaskSideEffects, useTaskLedgerActions } from './useTaskLedgerActions';

export interface UseNotificationEntriesInput {
  onOpenRecoveryCenter: () => void;
  onOpenAutomationSettings: () => void;
  onOpenSettingsTab?: (tab: SettingsTab) => void;
  onOpenModelSettings?: () => void;
  closePanel: () => void;
}

export interface UseNotificationEntriesResult {
  /** All entries, sorted by pinnedRank descending then timestamp descending. */
  entries: NotificationEntry[];
  /** Entries grouped by priority. */
  grouped: NotificationGroup;
  /** Badge count (action + active entries). */
  badgeCount: number;
  /** Number of succeeded ledger tasks (for the clear menu). */
  succeededCount: number;
  /** Total clearable ledger tasks (for the clear menu). */
  totalClearableCount: number;
  /** Clear succeeded or all non-active ledger tasks. */
  handleClear: (scope: 'succeeded' | 'all') => void;
}

function getBadgeLabel(count: number): string {
  return count > 9 ? '9+' : String(count);
}

export function useNotificationEntries({
  onOpenRecoveryCenter,
  onOpenAutomationSettings,
  onOpenSettingsTab,
  onOpenModelSettings,
  closePanel,
}: UseNotificationEntriesInput): UseNotificationEntriesResult {
  const { t } = useTranslation();

  // --- Store selectors ---
  const tasks = useTaskLedgerStore((state) => state.tasks);
  const clearSucceededTasks = useTaskLedgerStore((state) => state.clearSucceeded);
  const clearAllNonActiveTasks = useTaskLedgerStore((state) => state.clearAllNonActive);
  const isRecoveryLoaded = useRecoveryStore((state) => state.isLoaded);
  const config = useConfigStore((state) => state.config);
  const confirm = useDialogStore((state) => state.confirm);
  const modelDownloads = useModelDownloadStore((state) => state.downloads);
  const automationRules = useAutomationStore((state) => state.rules);
  const hasDegradedNotice = useSpeakerNotificationStore((state) => state.hasDegradedNotice);
  const degradedCount = useSpeakerNotificationStore((state) => state.degradedCount);
  const dismissSpeakerNotice = useSpeakerNotificationStore((state) => state.dismissNotice);

  const prevConfigRef = useRef(config);

  useEffect(() => {
    const prevConfig = prevConfigRef.current;
    prevConfigRef.current = config;

    if (hasDegradedNotice && config && prevConfig !== config) {
      if (
        !speakerService.isDiarizationDegraded(config, 'batch') &&
        !speakerService.isDiarizationDegraded(config, 'live')
      ) {
        dismissSpeakerNotice();
      }
    }
  }, [hasDegradedNotice, config, dismissSpeakerNotice]);

  const {
    isOpen: isOnboardingOpen,
    persistedState: onboardingState,
    dismissReminder: dismissOnboardingReminder,
    reopen: reopenOnboarding,
    modelDownloadStatus,
    modelDownloadProgress,
    modelDownloadError,
  } = useOnboardingStore(
    useShallow((state) => ({
      isOpen: state.isOpen,
      persistedState: state.persistedState,
      dismissReminder: state.dismissReminder,
      reopen: state.reopen,
      modelDownloadStatus: state.modelDownloadStatus,
      modelDownloadProgress: state.modelDownloadProgress,
      modelDownloadError: state.modelDownloadError,
    }))
  );

  const {
    status: updateStatus,
    updateInfo,
    progress: updateProgress,
    notificationVisible,
    installUpdate,
    dismissNotification: dismissUpdateNotification,
    relaunchToUpdate,
  } = useAppUpdater();

  // --- Onboarding handlers ---
  const handleDismissOnboarding = useCallback(async () => {
    const confirmed = await confirm(t('first_run.banner.dismiss_confirm_message'), {
      title: t('first_run.banner.dismiss_confirm_title'),
      variant: 'warning',
      confirmLabel: t('first_run.banner.dismiss_confirm_action'),
    });
    if (confirmed) {
      dismissOnboardingReminder();
    }
  }, [confirm, dismissOnboardingReminder, t]);

  const handleReopenOnboarding = useCallback(() => {
    reopenOnboarding(getResumeOnboardingStep(config, 'startup', onboardingState), 'startup');
  }, [config, onboardingState, reopenOnboarding]);

  const onboardActions = useMemo(
    () => ({
      reopen: handleReopenOnboarding,
      dismiss: () => {
        void handleDismissOnboarding();
      },
    }),
    [handleDismissOnboarding, handleReopenOnboarding]
  );

  // --- Action registry ---
  const taskActions = useTaskLedgerActions({
    t,
    onOpenRecoveryCenter,
    onOpenAutomationSettings,
    closePanel,
    updater: {
      installUpdate,
      dismissNotification: dismissUpdateNotification,
      relaunchToUpdate,
    },
    onboard: onboardActions,
  });

  // --- Ledger & download clear counts ---
  const succeededCount = useMemo(() => {
    const taskSucceeded = tasks.filter((task) => task.status === 'succeeded').length;
    const modelSucceeded = Object.values(modelDownloads).filter(
      (item) => item.status === 'completed'
    ).length;
    return taskSucceeded + modelSucceeded;
  }, [tasks, modelDownloads]);

  const totalClearableCount = useMemo(() => {
    const taskClearable = tasks.filter((task) => !isTaskLedgerActiveStatus(task.status)).length;
    const modelClearable = Object.values(modelDownloads).filter(
      (item) => item.status !== 'downloading'
    ).length;
    return taskClearable + modelClearable;
  }, [tasks, modelDownloads]);

  const handleClear = useCallback(
    (scope: 'succeeded' | 'all') => {
      if (scope === 'succeeded') {
        if (typeof useBatchQueueStore.getState === 'function') {
          useBatchQueueStore.getState().clearCompleted();
        }
        if (typeof useModelDownloadStore.getState === 'function') {
          useModelDownloadStore.getState().clearCompleted();
        }
        void clearSucceededTasks();
        return;
      }

      if (typeof useModelDownloadStore.getState === 'function') {
        useModelDownloadStore.getState().clearAllNonActive();
      }
      const recoveryStore =
        typeof useRecoveryStore.getState === 'function' ? useRecoveryStore.getState() : undefined;
      const batchQueueStore =
        typeof useBatchQueueStore.getState === 'function'
          ? useBatchQueueStore.getState()
          : undefined;
      const automationStore =
        typeof useAutomationStore.getState === 'function'
          ? useAutomationStore.getState()
          : undefined;

      const clearableTasks = tasks.filter((task) => !isTaskLedgerActiveStatus(task.status));
      for (const task of clearableTasks) {
        void cleanupLedgerTaskSideEffects(task, {
          discardRecoveryItem: recoveryStore?.discardItem,
          removeBatchQueueItem: batchQueueStore?.removeItem,
          dismissAutomationNotification: automationStore?.dismissNotification,
        });
      }
      void clearAllNonActiveTasks();
    },
    [clearAllNonActiveTasks, clearSucceededTasks, tasks]
  );

  // --- Entry aggregation ---
  const showOnboardingReminder = shouldShowOnboardingReminder(config, onboardingState);

  const ruleNameById = useMemo(() => {
    const map: Record<string, string> = {};
    if (Array.isArray(automationRules)) {
      for (const rule of automationRules) {
        if (rule?.id && rule?.name) {
          map[rule.id] = rule.name;
        }
      }
    }
    return map;
  }, [automationRules]);

  const entries = useMemo<NotificationEntry[]>(() => {
    const now = Date.now();
    const result: NotificationEntry[] = [];

    // 1. Task ledger entries
    result.push(
      ...adaptTaskLedgerEntries(tasks, isRecoveryLoaded, t, taskActions, {
        ruleNameById,
        now,
      })
    );

    // 2. App update entry
    const updateEntry = adaptUpdateEntry(
      updateStatus,
      updateInfo,
      updateProgress,
      notificationVisible,
      t,
      taskActions
    );
    if (updateEntry) {
      result.push(updateEntry);
    }

    // 3. Onboarding reminder entry
    const onboardingEntry = adaptOnboardingEntry(
      isOnboardingOpen,
      showOnboardingReminder,
      t,
      taskActions
    );
    if (onboardingEntry) {
      result.push(onboardingEntry);
    }

    // 4. Onboarding model download entry
    const downloadEntry = adaptModelDownloadEntry(
      isOnboardingOpen,
      {
        status: modelDownloadStatus,
        progress: modelDownloadProgress,
        error: modelDownloadError,
      },
      t,
      {
        reopenOnboarding: handleReopenOnboarding,
        setModelDownloadIdle: () => {
          useOnboardingStore.getState().setModelDownloadStatus('idle');
        },
      }
    );
    if (downloadEntry) {
      result.push(downloadEntry);
    }

    // 5. Model downloads (Settings / Background)
    const modelDownloadEntries = adaptModelDownloadsEntries(
      modelDownloads,
      t,
      {
        cancelDownload: (modelId, downloadId) =>
          void useModelDownloadStore.getState().cancelDownload(modelId, downloadId),
        dismissDownload: (id) => useModelDownloadStore.getState().dismissDownload(id),
      },
      { now }
    );
    result.push(...modelDownloadEntries);

    // 6. Speaker diarization degraded notification
    const speakerDegradedEntry = adaptSpeakerDegradedEntry(
      hasDegradedNotice,
      t,
      () => {
        closePanel();
        if (onOpenSettingsTab) {
          onOpenSettingsTab('models');
        } else if (onOpenModelSettings) {
          onOpenModelSettings();
        }
      },
      dismissSpeakerNotice,
      degradedCount
    );
    if (speakerDegradedEntry) {
      result.push(speakerDegradedEntry);
    }

    // Sort by pinnedRank descending first, then timestamp descending (newest first)
    result.sort((a, b) => {
      const rankDiff = (b.pinnedRank ?? 0) - (a.pinnedRank ?? 0);
      if (rankDiff !== 0) {
        return rankDiff;
      }
      return b.timestamp - a.timestamp;
    });

    return result;
  }, [
    handleReopenOnboarding,
    isOnboardingOpen,
    isRecoveryLoaded,
    modelDownloadError,
    modelDownloadProgress,
    modelDownloadStatus,
    modelDownloads,
    notificationVisible,
    ruleNameById,
    showOnboardingReminder,
    t,
    taskActions,
    tasks,
    updateInfo,
    updateProgress,
    updateStatus,
    closePanel,
    degradedCount,
    dismissSpeakerNotice,
    hasDegradedNotice,
    onOpenModelSettings,
    onOpenSettingsTab,
  ]);

  // --- Grouped entries ---
  const grouped = useMemo<NotificationGroup>(
    () => ({
      action: entries.filter((e) => e.priority === 'action'),
      active: entries.filter((e) => e.priority === 'active'),
      info: entries.filter((e) => e.priority === 'info'),
    }),
    [entries]
  );

  const badgeCount = grouped.action.length + grouped.active.length;

  return {
    entries,
    grouped,
    badgeCount,
    succeededCount,
    totalClearableCount,
    handleClear,
  };
}

export { getBadgeLabel };
