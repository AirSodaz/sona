import { ModelIcon } from '../../components/Icons';
import type { NotificationEntry } from '../../types/notification';
import { NOTIFICATION_PINNED_RANK, type TFunction } from './adapterUtils';

export function adaptSpeakerDegradedEntry(
  hasNotice: boolean,
  t: TFunction,
  onGoToSettings: () => void,
  onDismiss: () => void,
  degradedCount = 1
): NotificationEntry | null {
  if (!hasNotice) {
    return null;
  }

  const body =
    degradedCount > 1
      ? t('notifications.speaker_degraded_body_count', {
          count: degradedCount,
          defaultValue:
            'Speaker diarization was skipped for {{count}} tasks because required models are not configured. Transcription completed normally.',
        })
      : t('notifications.speaker_degraded_body', {
          defaultValue:
            'Speaker diarization was requested but required models are not configured. Transcription completed normally. Configure models in Settings to enable speaker labels.',
        });

  return {
    id: 'speaker-missing-model',
    source: 'task',
    priority: 'action',
    tone: 'warning',
    icon: <ModelIcon />,
    title: t('notifications.speaker_degraded_title', {
      defaultValue: 'Speaker Diarization Skipped',
    }),
    body,
    timestamp: 0,
    pinnedRank: NOTIFICATION_PINNED_RANK.SPEAKER_DEGRADED,
    actions: [
      {
        id: 'go-to-settings',
        label: t('common.go_to_settings', { defaultValue: 'Go to Settings' }),
        variant: 'primary',
        run: onGoToSettings,
      },
    ],
    closeAction: {
      id: 'dismiss-speaker-degraded',
      label: t('common.dismiss', { defaultValue: 'Dismiss' }),
      variant: 'soft',
      run: onDismiss,
    },
    itemClassName: 'notification-center-item-speaker-degraded',
  };
}
