import { CheckIcon, DownloadIcon } from '../../components/Icons';
import type { ModelDownloadItem } from '../../stores/modelDownloadStore';
import type {
  NotificationAction,
  NotificationEntry,
  NotificationPriority,
  NotificationTone,
} from '../../types/notification';
import type { TFunction } from './adapterUtils';

export function formatBytes(bytes?: number): string {
  if (!bytes || bytes <= 0) return '';
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(0)} MB`;
}

export interface ModelDownloadAdapterCallbacks {
  cancelDownload: (modelId: string, downloadId?: string) => Promise<unknown> | undefined;
  dismissDownload: (modelId: string) => void;
}

export function adaptModelDownloadsEntries(
  downloads: Record<string, ModelDownloadItem>,
  t: TFunction,
  callbacks: ModelDownloadAdapterCallbacks
): NotificationEntry[] {
  const entries: NotificationEntry[] = [];

  for (const item of Object.values(downloads)) {
    const isActive = item.status === 'downloading';
    const isSuccess = item.status === 'completed';
    const isFailed = item.status === 'failed';
    const isCancelled = item.status === 'cancelled';

    let priority: NotificationPriority = 'info';
    let tone: NotificationTone = 'accent';

    if (isActive) {
      priority = 'active';
      tone = 'accent';
    } else if (isFailed) {
      priority = 'action';
      tone = 'error';
    } else if (isSuccess) {
      priority = 'info';
      tone = 'success';
    } else if (isCancelled) {
      priority = 'info';
      tone = 'info';
    }

    const title = isActive
      ? t('notifications.model_download.downloading_title', {
          name: item.modelName,
          defaultValue: `Downloading ${item.modelName}…`,
        })
      : isSuccess
        ? t('notifications.model_download.completed_title', {
            name: item.modelName,
            defaultValue: `${item.modelName} ready`,
          })
        : isFailed
          ? t('notifications.model_download.failed_title', {
              name: item.modelName,
              defaultValue: `${item.modelName} download failed`,
            })
          : t('notifications.model_download.cancelled_title', {
              name: item.modelName,
              defaultValue: `${item.modelName} download cancelled`,
            });

    let body: string | null = null;
    if (isActive) {
      const parts: string[] = [];
      if (item.downloadedBytes && item.totalBytes) {
        parts.push(`${formatBytes(item.downloadedBytes)} / ${formatBytes(item.totalBytes)}`);
      }
      if (item.speed) {
        parts.push(item.speed);
      }
      body = parts.length > 0 ? parts.join(' • ') : `${item.progress}%`;
    } else if (isSuccess) {
      body = t('notifications.model_download.completed_body', {
        defaultValue: 'Model installed and ready to use.',
      });
    } else if (isFailed) {
      body =
        item.error ||
        t('notifications.model_download.failed_body', {
          defaultValue: 'Failed to complete model download.',
        });
    }

    const actions: NotificationAction[] = [];
    if (isActive) {
      actions.push({
        id: `cancel-${item.modelId}`,
        label: t('common.cancel', { defaultValue: 'Cancel' }),
        variant: 'soft',
        run: () => {
          void callbacks.cancelDownload(item.modelId, item.downloadId);
        },
      });
    } else if (isFailed && item.onRetry) {
      actions.push({
        id: `retry-${item.modelId}`,
        label: t('common.retry', { defaultValue: 'Retry' }),
        variant: 'primary',
        run: () => {
          if (item.onRetry) {
            void item.onRetry();
          }
        },
      });
    }

    const closeAction: NotificationAction | undefined = !isActive
      ? {
          id: `close-${item.modelId}`,
          label: t('common.close', { defaultValue: 'Close' }),
          variant: 'soft',
          run: () => callbacks.dismissDownload(item.modelId),
        }
      : undefined;

    entries.push({
      id: `model-dl-${item.modelId}`,
      source: 'download',
      priority,
      tone,
      icon: isSuccess ? <CheckIcon /> : <DownloadIcon />,
      title,
      body,
      timestamp: item.timestamp,
      progress: isActive ? item.progress : undefined,
      actions,
      closeAction,
      itemClassName: `notification-center-item-download${isFailed ? ' notification-center-item-error' : ''}`,
    });
  }

  return entries;
}
