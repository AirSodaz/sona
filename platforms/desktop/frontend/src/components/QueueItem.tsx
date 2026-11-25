import type { TFunction } from 'i18next';
import { Globe, Tv } from 'lucide-react';
import { memo, useState } from 'react';
import { cancelBatchTask } from '../services/tauri/recognizer';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useDialogStore } from '../stores/dialogStore';
import { useProjectStore } from '../stores/projectStore';
import type { BatchQueueItem, BatchQueueItemStatus } from '../types/batchQueue';
import type { ProjectRecord } from '../types/project';
import { logger } from '../utils/logger';
import {
  CancelledIcon,
  CompleteIcon,
  ErrorIcon,
  PendingIcon,
  ProcessingIcon,
  RestoreIcon,
  XIcon,
} from './Icons';
import { IconButton } from './ui/Button';
import { Tooltip } from './ui/Tooltip';

/** Returns the icon corresponding to a queue item's status. */
export const getQueueStatusIcon = (status: BatchQueueItemStatus): React.JSX.Element => {
  switch (status) {
    case 'pending':
      return <PendingIcon />;
    case 'processing':
      return <ProcessingIcon />;
    case 'complete':
      return <CompleteIcon />;
    case 'error':
      return <ErrorIcon />;
    case 'cancelled':
      return <CancelledIcon />;
    default:
      return <PendingIcon />;
  }
};

const VIDEO_MEDIA_EXTENSIONS = [
  '.mp4',
  '.webm',
  '.mov',
  '.mkv',
  '.avi',
  '.wmv',
  '.flv',
  '.3gp',
  '.m4v',
  '.ts',
  '.mts',
  '.m2ts',
];

export function isTranscodeOrFormatFailure(item: {
  filename?: string;
  filePath?: string;
  errorMessage?: string;
}): boolean {
  const target = (item.filename || item.filePath || '').toLowerCase();
  if (VIDEO_MEDIA_EXTENSIONS.some((ext) => target.endsWith(ext))) {
    return true;
  }
  const errorMsg = (item.errorMessage || '').toLowerCase();
  return (
    errorMsg.includes('decode') ||
    errorMsg.includes('transcode') ||
    errorMsg.includes('ffmpeg') ||
    errorMsg.includes('format') ||
    errorMsg.includes('container') ||
    errorMsg.includes('codec') ||
    errorMsg.includes('unsupported') ||
    errorMsg.includes('转码') ||
    errorMsg.includes('解码') ||
    errorMsg.includes('symphonia')
  );
}

export interface QueueItemProps {
  item: BatchQueueItem;
  isActive: boolean;
  project?: ProjectRecord;
  onActivate: (id: string) => void;
  onRemove: (id: string) => void;
  onRetry?: (id: string) => void;
  t: TFunction;
}

/** Individual queue item row with progress, metadata, and quick actions. */
export const QueueItem = memo(function QueueItem({
  item,
  isActive,
  project,
  onActivate,
  onRemove,
  onRetry,
  t,
}: QueueItemProps): React.JSX.Element {
  const [isFinishing, setIsFinishing] = useState(false);
  const mediaContext = item.mediaServerContext ?? item.embyContext;
  const isMediaServerUploadError =
    item.status === 'error' && Boolean(mediaContext && item.errorStage === 'upload');
  const isTranscodeError = item.status === 'error' && isTranscodeOrFormatFailure(item);
  const rawErrorMessage = item.errorMessage || t('batch.file_failed');
  const displayErrorMessage = isMediaServerUploadError
    ? t('batch.media_server_upload_failed', {
        defaultValue: 'Subtitle upload to media server failed',
      })
    : isTranscodeError &&
        (rawErrorMessage.startsWith('Failed to decode audio file') ||
          rawErrorMessage.startsWith('Failed to extract audio slice'))
      ? t('batch.transcode_failed', { defaultValue: 'Transcode failed' })
      : rawErrorMessage;
  const retryLabel = isMediaServerUploadError
    ? t('batch.retry_upload_only', { defaultValue: 'Retry Subtitle Upload' })
    : t('batch.retry', {
        defaultValue: t('common.retry', { defaultValue: 'Retry' }),
      });

  const handleClick = () => {
    onActivate(item.id);
  };

  const handleRemove = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (item.status === 'processing') {
      const confirmed = await useDialogStore.getState().confirm(
        t('batch.cancel_confirm_message', {
          filename: item.filename,
          progress: Math.round(item.progress ?? 0),
          defaultValue: `"${item.filename}" is currently being transcribed (${Math.round(item.progress ?? 0)}%). Stopping will discard current progress. Are you sure you want to stop and remove this file?`,
        }),
        {
          title: t('batch.cancel_confirm_title', { defaultValue: 'Stop Transcription?' }),
          variant: 'warning',
          confirmLabel: t('batch.stop_transcription', { defaultValue: 'Stop & Remove' }),
          cancelLabel: t('batch.continue_transcribing', { defaultValue: 'Continue Transcribing' }),
        }
      );
      if (!confirmed) {
        return;
      }
    }
    onRemove(item.id);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget) {
      return;
    }
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onActivate(item.id);
    }
  };

  return (
    <div
      className={`queue-item queue-item-${item.status} ${isActive ? 'queue-item-active' : ''}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="listitem"
      tabIndex={0}
      aria-current={isActive ? 'true' : undefined}
      aria-label={`${item.filename} - ${t(`batch.status_${item.status}`)}`}
    >
      <div className="queue-item-icon" aria-hidden="true">
        {getQueueStatusIcon(item.status)}
      </div>

      <div className="queue-item-content">
        <div className="queue-item-header">
          <Tooltip content={item.filename} placement="top" onlyWhenTruncated>
            <div className="queue-item-filename">{item.filename}</div>
          </Tooltip>
          {item.status === 'processing' &&
            (mediaContext && !item.streamUrl && !item.filePath ? (
              <span
                className="queue-item-emby-pulling"
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  fontSize: '11px',
                  color: 'var(--color-info)',
                  whiteSpace: 'nowrap',
                  flexShrink: 0,
                }}
              >
                {t('emby.pulling_audio', { defaultValue: 'Fetching audio track...' })}
              </span>
            ) : (
              <span className="queue-item-progress-pct" aria-hidden="true">
                {Math.round(item.progress)}%
              </span>
            ))}
        </div>

        {item.status === 'processing' && (
          <div
            className="queue-item-progress"
            role="progressbar"
            aria-valuenow={Math.round(item.progress ?? 0)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={item.filename}
          >
            <div className="queue-item-progress-fill" style={{ width: `${item.progress ?? 0}%` }} />
          </div>
        )}

        <div className="queue-item-meta">
          {mediaContext && (
            <Tooltip content={mediaContext.itemName} placement="bottom">
              <span
                className="queue-item-media-server-badge"
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  fontSize: '11px',
                  whiteSpace: 'nowrap',
                  flexShrink: 0,
                  color:
                    mediaContext.serverType === 'jellyfin'
                      ? '#a855f7'
                      : 'var(--color-success, #22c55e)',
                  backgroundColor:
                    mediaContext.serverType === 'jellyfin'
                      ? 'rgba(168, 85, 247, 0.1)'
                      : 'rgba(34, 197, 94, 0.1)',
                  padding: '1px 6px',
                  borderRadius: '4px',
                }}
              >
                <Tv size={11} strokeWidth={2} />
                <span>{mediaContext.serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'}</span>
              </span>
            </Tooltip>
          )}
          {item.sourceType === 'url_stream' && (
            <Tooltip
              content={
                item.streamUrl
                  ? item.streamUrl.replace(/([?&]api_key=)[^&#]+/gi, '$1***')
                  : item.filePath
              }
              placement="bottom"
              multiline
            >
              <span
                className="queue-item-stream-badge"
                tabIndex={0}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  fontSize: '11px',
                  whiteSpace: 'nowrap',
                  flexShrink: 0,
                  color: 'var(--color-info)',
                  backgroundColor: 'color-mix(in srgb, var(--color-info) 12%, transparent)',
                  padding: '1px 6px',
                  borderRadius: '4px',
                }}
              >
                <Globe size={11} strokeWidth={2} />
                <span>{t('batch.stream_badge', { defaultValue: 'Network Stream' })}</span>
              </span>
            </Tooltip>
          )}
          {project && (
            <span className="queue-item-project-badge">
              <span
                className="queue-item-project-dot"
                style={{ backgroundColor: project.color || 'var(--color-text-muted)' }}
                aria-hidden="true"
              />
              <span className="queue-item-project-name">{project.name}</span>
            </span>
          )}
          {item.origin === 'automation' && (
            <Tooltip
              content={
                item.automationRuleName || t('automation.automated', { defaultValue: 'Automated' })
              }
              placement="bottom"
              onlyWhenTruncated
            >
              <span className="queue-item-automation">
                {t('automation.automated', { defaultValue: 'Automated' })}
                {item.automationRuleName ? ` · ${item.automationRuleName}` : ''}
              </span>
            </Tooltip>
          )}

          {item.status === 'error' && (
            <Tooltip content={rawErrorMessage} placement="bottom" multiline>
              <span
                className="queue-item-error"
                tabIndex={0}
                role="note"
                aria-label={rawErrorMessage}
              >
                <span className="queue-item-error-msg">{displayErrorMessage}</span>
              </span>
            </Tooltip>
          )}

          {item.status === 'cancelled' && (
            <span className="queue-item-cancelled">{t('batch.status_cancelled')}</span>
          )}
        </div>
      </div>

      <div className="queue-item-actions">
        {item.sourceType === 'url_stream' && !mediaContext && item.status === 'processing' && (
          <IconButton
            tier="ghost"
            size="xs"
            className="queue-item-finish-save"
            icon={<CompleteIcon width={13} height={13} />}
            disabled={!item.activeInstanceId || isFinishing}
            onClick={async (e) => {
              e.stopPropagation();
              if (!item.activeInstanceId || isFinishing) return;
              setIsFinishing(true);
              try {
                await cancelBatchTask(item.activeInstanceId);
              } catch (err) {
                logger.error('[QueueItem] Failed to finish stream task:', err);
                setIsFinishing(false);
              }
            }}
            aria-label={t('batch.finish_and_save', { defaultValue: 'Finish and Save' })}
            tooltipPos="left"
            style={{ color: 'var(--color-success, #22c55e)' }}
          />
        )}
        {(item.status === 'error' || item.status === 'cancelled') && (
          <IconButton
            tier="ghost"
            size="xs"
            className="queue-item-retry"
            icon={<RestoreIcon width={13} height={13} />}
            onClick={(e) => {
              e.stopPropagation();
              onRetry?.(item.id);
            }}
            aria-label={retryLabel}
            tooltipPos="left"
          />
        )}

        <IconButton
          tier="ghost"
          tone="danger"
          size="xs"
          className="queue-item-remove"
          icon={<XIcon width={12} height={12} />}
          onClick={handleRemove}
          aria-label={t('common.delete_item', { item: item.filename })}
          tooltip={t('common.delete')}
          tooltipPos="left"
        />
      </div>
    </div>
  );
});

/**
 * Isolated subscription container for a single queue item.
 * Prevents unnecessary re-renders of the full list when only one item's progress updates.
 */
export function QueueItemContainer({
  id,
  t,
}: {
  id: string;
  t: TFunction;
}): React.JSX.Element | null {
  const item = useBatchQueueStore((state) => state.queueItems.find((i) => i.id === id));
  const isActive = useBatchQueueStore((state) => state.activeItemId === id);
  const setActiveItem = useBatchQueueStore((state) => state.setActiveItem);
  const removeItem = useBatchQueueStore((state) => state.removeItem);
  const retryItem = useBatchQueueStore((state) => state.retryItem);
  const project = useProjectStore((state) =>
    item?.projectId ? state.projects.find((p) => p.id === item.projectId) : undefined
  );

  if (!item) return null;

  return (
    <QueueItem
      item={item}
      isActive={isActive}
      onActivate={setActiveItem}
      onRemove={removeItem}
      onRetry={retryItem}
      project={project}
      t={t}
    />
  );
}
