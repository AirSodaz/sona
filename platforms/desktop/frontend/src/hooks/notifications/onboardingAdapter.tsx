import { CheckIcon, DownloadIcon, SparklesIcon } from '../../components/Icons';
import type { NotificationAction, NotificationEntry } from '../../types/notification';
import type { TaskCenterActionRegistry } from '../useTaskLedgerActions';
import { type TFunction, toNotificationAction } from './adapterUtils';

/**
 * Produces the "Complete Setup" onboarding reminder entry when the user hasn't
 * finished the first-run wizard and the reminder hasn't been dismissed.
 */
export function adaptOnboardingEntry(
  isOnboardingOpen: boolean,
  shouldShow: boolean,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry
): NotificationEntry | null {
  if (isOnboardingOpen || !shouldShow) {
    return null;
  }

  const registryActions = actionRegistry.getOnboardingReminderActions();

  return {
    id: 'onboarding',
    source: 'onboarding',
    priority: 'action',
    tone: 'accent',
    icon: <SparklesIcon />,
    title: t('first_run.banner.title'),
    body: t('first_run.banner.body'),
    timestamp: Number.MAX_SAFE_INTEGER,
    actions: registryActions.row.map(toNotificationAction),
    closeAction: registryActions.close ? toNotificationAction(registryActions.close) : undefined,
    itemClassName: 'notification-center-item-onboarding',
  };
}

interface ModelDownloadState {
  status: 'idle' | 'downloading' | 'completed' | 'failed';
  progress: number;
  error: string;
}

interface ModelDownloadCallbacks {
  reopenOnboarding: () => void;
  setModelDownloadIdle: () => void;
}

/**
 * Produces a notification entry for background model downloads (downloading,
 * completed, or failed).
 */
export function adaptModelDownloadEntry(
  isOnboardingOpen: boolean,
  download: ModelDownloadState,
  t: TFunction,
  callbacks: ModelDownloadCallbacks
): NotificationEntry | null {
  if (isOnboardingOpen || download.status === 'idle') {
    return null;
  }

  const isActive = download.status === 'downloading';
  const isSuccess = download.status === 'completed';
  const isError = download.status === 'failed';

  const title = isActive
    ? t('first_run.download_notification.downloading_title', {
        defaultValue: 'Downloading models…',
      })
    : isSuccess
      ? t('first_run.download_notification.completed_title', {
          defaultValue: 'Models ready',
        })
      : t('first_run.download_notification.failed_title', {
          defaultValue: 'Model download failed',
        });

  const body = isActive
    ? t('first_run.download_notification.downloading_body', {
        defaultValue: 'Recommended models are being downloaded in the background.',
      })
    : isSuccess
      ? t('first_run.download_notification.completed_body', {
          defaultValue: 'Local transcription models are installed and ready to use.',
        })
      : download.error ||
        t('first_run.download_notification.failed_body', {
          defaultValue: 'Could not finish downloading the recommended models.',
        });

  const actions: NotificationAction[] = [];

  if (isError) {
    actions.push({
      id: 'retry',
      label: t('first_run.download_notification.retry', { defaultValue: 'Retry' }),
      variant: 'primary',
      run: callbacks.reopenOnboarding,
    });
  } else if (isSuccess) {
    actions.push({
      id: 'dismiss',
      label: t('first_run.download_notification.dismiss', { defaultValue: 'Dismiss' }),
      variant: 'soft',
      run: callbacks.setModelDownloadIdle,
    });
  }

  return {
    id: 'onboarding-download',
    source: 'download',
    priority: isActive ? 'active' : 'info',
    tone: isError ? 'error' : isSuccess ? 'success' : 'accent',
    icon: isSuccess ? <CheckIcon /> : <DownloadIcon />,
    title,
    body,
    timestamp: Number.MAX_SAFE_INTEGER,
    progress: isActive ? download.progress : undefined,
    actions,
    itemClassName: `notification-center-item-download${isError ? ' notification-center-item-error' : ''}`,
  };
}
