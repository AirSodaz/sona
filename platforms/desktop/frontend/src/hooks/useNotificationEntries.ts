import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import type { NotificationEntry, NotificationGroup } from '../types/notification';
import { getResumeOnboardingStep, shouldShowOnboardingReminder } from '../utils/onboarding';
import { adaptModelDownloadEntry, adaptOnboardingEntry } from './notifications/onboardingAdapter';
import { adaptTaskLedgerEntries } from './notifications/taskLedgerAdapter';
import { adaptUpdateEntry } from './notifications/updateAdapter';
import { useAppUpdater } from './useAppUpdater';
import { useTaskLedgerActions } from './useTaskLedgerActions';

export interface UseNotificationEntriesInput {
  onOpenRecoveryCenter: () => void;
  onOpenAutomationSettings: () => void;
  closePanel: () => void;
}

export interface UseNotificationEntriesResult {
  /** All entries, sorted by timestamp descending. */
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

  // --- Onboarding dismiss handler ---
  const handleDismissOnboarding = async () => {
    const confirmed = await confirm(t('first_run.banner.dismiss_confirm_message'), {
      title: t('first_run.banner.dismiss_confirm_title'),
      variant: 'warning',
      confirmLabel: t('first_run.banner.dismiss_confirm_action'),
    });
    if (confirmed) {
      dismissOnboardingReminder();
    }
  };

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
    onboard: {
      reopen: () =>
        reopenOnboarding(getResumeOnboardingStep(config, 'startup', onboardingState), 'startup'),
      dismiss: () => {
        void handleDismissOnboarding();
      },
    },
  });

  // --- Ledger clear counts ---
  const succeededCount = useMemo(
    () => tasks.filter((task) => task.status === 'succeeded').length,
    [tasks]
  );
  const totalClearableCount = useMemo(() => {
    const activeStatuses: Record<string, true> = {
      pending: true,
      running: true,
      cancelRequested: true,
    };
    return tasks.filter((task) => !activeStatuses[task.status]).length;
  }, [tasks]);

  const handleClear = (scope: 'succeeded' | 'all') => {
    if (scope === 'succeeded') {
      void clearSucceededTasks();
    } else {
      void clearAllNonActiveTasks();
    }
  };

  // --- Entry aggregation ---
  const showOnboardingReminder = shouldShowOnboardingReminder(config, onboardingState);

  const entries = useMemo<NotificationEntry[]>(() => {
    const result: NotificationEntry[] = [];

    // 1. Task ledger entries
    result.push(...adaptTaskLedgerEntries(tasks, isRecoveryLoaded, t, taskActions));

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

    // 4. Model download entry
    const downloadEntry = adaptModelDownloadEntry(
      isOnboardingOpen,
      {
        status: modelDownloadStatus,
        progress: modelDownloadProgress,
        error: modelDownloadError,
      },
      t,
      {
        reopenOnboarding: () =>
          reopenOnboarding(getResumeOnboardingStep(config, 'startup', onboardingState), 'startup'),
        setModelDownloadIdle: () => {
          useOnboardingStore.getState().setModelDownloadStatus('idle');
        },
      }
    );
    if (downloadEntry) {
      result.push(downloadEntry);
    }

    // Sort by timestamp descending (newest first)
    result.sort((a, b) => b.timestamp - a.timestamp);

    return result;
  }, [
    config,
    isOnboardingOpen,
    isRecoveryLoaded,
    modelDownloadError,
    modelDownloadProgress,
    modelDownloadStatus,
    notificationVisible,
    onboardingState,
    reopenOnboarding,
    showOnboardingReminder,
    t,
    taskActions,
    tasks,
    updateInfo,
    updateProgress,
    updateStatus,
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
