import { CheckIcon, DownloadIcon } from '../../components/Icons';
import type { Update } from '../../services/tauri/platform/updater';
import type { UpdateStatus } from '../../stores/appUpdaterStore';
import type { NotificationEntry, NotificationPriority } from '../../types/notification';
import type { TaskCenterActionRegistry } from '../useTaskLedgerActions';
import { type TFunction, toNotificationAction } from './adapterUtils';

/**
 * Produces a NotificationEntry for the app-update notification when visible,
 * or null when the notification is dismissed or no update is available.
 */
export function adaptUpdateEntry(
  status: UpdateStatus,
  updateInfo: Update | null,
  progress: number,
  notificationVisible: boolean,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry
): NotificationEntry | null {
  if (!notificationVisible || !updateInfo) {
    return null;
  }

  const isBusy = status === 'downloading' || status === 'installing';
  const priority: NotificationPriority = isBusy ? 'active' : 'action';

  const registryActions = actionRegistry.getUpdateTaskActions({ status, isBusy });

  const body = status === 'available' ? updateInfo.body || t('settings.update_desc_default') : null;

  // "Ready to relaunch" status badge
  let statusBadge: React.ReactNode | undefined;
  if (status === 'downloaded') {
    statusBadge = (
      <div className="update-status success notification-center-update-status">
        <CheckIcon />
        <span>{t('settings.update_relaunch')}</span>
      </div>
    );
  }

  return {
    id: 'update',
    source: 'update',
    priority,
    tone: 'accent',
    icon: <DownloadIcon />,
    title: t('settings.update_available', { version: updateInfo.version }),
    body,
    bodyClassName: 'notification-center-update-body',
    timestamp: Number.MAX_SAFE_INTEGER,
    progress: isBusy ? progress : undefined,
    actions: registryActions.row.map(toNotificationAction),
    closeAction: registryActions.close ? toNotificationAction(registryActions.close) : undefined,
    support: statusBadge,
    itemClassName: 'notification-center-item-update',
  };
}
