import type { NotificationAction } from '../../types/notification';

export type TFunction = (key: string, options?: Record<string, unknown>) => string;

/**
 * Deterministic priority ranks for pinned system/reminder notifications.
 * Higher values appear above lower values; regular timestamped tasks use rank 0.
 */
export const NOTIFICATION_PINNED_RANK = {
  ONBOARDING: 4,
  UPDATE: 3,
  ONBOARDING_DOWNLOAD: 2,
  SPEAKER_DEGRADED: 1,
  DEFAULT: 0,
} as const;

const MIN_REAL_TIMESTAMP_MS = 1_000_000_000_000;

/**
 * Formats a Unix millisecond timestamp as a short relative label or MM-DD HH:MM clock label.
 * Returns null for synthetic non-epoch test timestamps.
 */
export function formatNotificationTimestamp(
  timestamp: number,
  now: number,
  t: TFunction
): string | null {
  if (!Number.isFinite(timestamp) || timestamp < MIN_REAL_TIMESTAMP_MS) {
    return null;
  }
  const diffMs = Math.max(0, now - timestamp);
  if (diffMs < 60_000) {
    return t('task_center.time_just_now', { defaultValue: 'Just now' });
  }
  if (diffMs < 3_600_000) {
    const minutes = Math.floor(diffMs / 60_000);
    return t('task_center.time_minutes_ago', {
      count: minutes,
      defaultValue: '{{count}}m ago',
    });
  }
  if (diffMs < 86_400_000) {
    const hours = Math.floor(diffMs / 3_600_000);
    return t('task_center.time_hours_ago', {
      count: hours,
      defaultValue: '{{count}}h ago',
    });
  }
  const date = new Date(timestamp);
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hour = String(date.getHours()).padStart(2, '0');
  const minute = String(date.getMinutes()).padStart(2, '0');
  return `${month}-${day} ${hour}:${minute}`;
}

export function createCloseNotificationAction(
  t: TFunction,
  run: () => void | Promise<void>
): NotificationAction {
  return {
    id: 'close',
    label: t('common.close', { defaultValue: 'Close' }),
    variant: 'soft',
    run,
  };
}
