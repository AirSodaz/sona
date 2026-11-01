import { ModelIcon } from '../../components/Icons';
import type { NotificationEntry } from '../../types/notification';
import type { TFunction } from './adapterUtils';

export function adaptSpeakerDegradedEntry(
  hasNotice: boolean,
  _degradedCount: number,
  t: TFunction,
  onGoToSettings: () => void,
  onDismiss: () => void
): NotificationEntry | null {
  if (!hasNotice) {
    return null;
  }

  return {
    id: 'speaker-missing-model',
    source: 'task',
    priority: 'action',
    tone: 'warning',
    icon: <ModelIcon />,
    title: t('notifications.speaker_degraded_title', {
      defaultValue: 'Speaker Diarization Skipped',
    }),
    body: t('notifications.speaker_degraded_body', {
      defaultValue:
        'Speaker diarization was requested but required models are not configured. Transcription completed normally.',
    }),
    timestamp: Date.now(),
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
      variant: 'secondary',
      run: onDismiss,
    },
    itemClassName: 'notification-center-item-speaker-degraded',
  };
}
