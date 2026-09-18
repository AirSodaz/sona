import type { TFunction } from 'i18next';
import { memo } from 'react';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useProjectStore } from '../stores/projectStore';
import type { BatchQueueItem, BatchQueueItemStatus } from '../types/batchQueue';
import type { ProjectRecord } from '../types/project';
import {
  CancelledIcon,
  CompleteIcon,
  ErrorIcon,
  PendingIcon,
  ProcessingIcon,
  RestoreIcon,
  XIcon,
} from './Icons';

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
  const handleClick = () => {
    onActivate(item.id);
  };

  const handleRemove = (e: React.MouseEvent) => {
    e.stopPropagation();
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
          <div className="queue-item-filename" title={item.filename}>
            {item.filename}
          </div>
          {item.status === 'processing' && (
            <span className="queue-item-progress-pct" aria-hidden="true">
              {Math.round(item.progress)}%
            </span>
          )}
        </div>

        {item.status === 'processing' && (
          <div
            className="queue-item-progress"
            role="progressbar"
            aria-valuenow={Math.round(item.progress)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={item.filename}
          >
            <div className="queue-item-progress-fill" style={{ width: `${item.progress}%` }} />
          </div>
        )}

        <div className="queue-item-meta">
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
            <span
              className="queue-item-automation"
              title={
                item.automationRuleName || t('automation.automated', { defaultValue: 'Automated' })
              }
            >
              {t('automation.automated', { defaultValue: 'Automated' })}
              {item.automationRuleName ? ` · ${item.automationRuleName}` : ''}
            </span>
          )}

          {item.status === 'error' && (
            <span className="queue-item-error" title={item.errorMessage || t('batch.file_failed')}>
              {item.errorMessage || t('batch.file_failed')}
            </span>
          )}

          {item.status === 'cancelled' && (
            <span className="queue-item-cancelled">{t('batch.status_cancelled')}</span>
          )}
        </div>
      </div>

      <div className="queue-item-actions">
        {(item.status === 'error' || item.status === 'cancelled') && (
          <button
            className="btn btn-secondary-soft btn-xs queue-item-retry"
            onClick={(e) => {
              e.stopPropagation();
              onRetry?.(item.id);
            }}
            aria-label={t('common.retry')}
            data-tooltip={t('common.retry')}
            data-tooltip-pos="left"
          >
            <RestoreIcon width={12} height={12} />
            <span className="queue-item-retry-text">{t('common.retry')}</span>
          </button>
        )}

        <button
          className="btn btn-icon queue-item-remove"
          onClick={handleRemove}
          aria-label={t('common.delete_item', { item: item.filename })}
          data-tooltip={t('common.delete')}
          data-tooltip-pos="left"
        >
          <XIcon width={12} height={12} />
        </button>
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
