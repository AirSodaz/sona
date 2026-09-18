import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { RestoreIcon } from './Icons';
import { QueueGroup } from './QueueGroup';
import { QueueItemContainer } from './QueueItem';

export interface QueueGroupedListProps {
  className?: string;
}

/**
 * Main batch queue list that partitions items into clear status groups:
 * - Processing (with active spinners & progress bars)
 * - Waiting (with project assignment selectors)
 * - Completed (collapsible)
 * - Failed (with batch retry controls)
 */
export function QueueGroupedList({
  className = '',
}: QueueGroupedListProps): React.JSX.Element | null {
  const { t } = useTranslation();

  const queueItems = useBatchQueueStore((state) => state.queueItems);
  const retryAllFailed = useBatchQueueStore((state) => state.retryAllFailed);

  if (queueItems.length === 0) {
    return null;
  }

  const processingItems = queueItems.filter((item) => item.status === 'processing');
  const pendingItems = queueItems.filter((item) => item.status === 'pending');
  const completeItems = queueItems.filter((item) => item.status === 'complete');
  const failedItems = queueItems.filter(
    (item) => item.status === 'error' || item.status === 'cancelled'
  );

  return (
    <div
      className={`queue-grouped-list ${className}`}
      role="list"
      aria-label={t('batch.queue_title', { count: queueItems.length })}
    >
      {/* 1. Processing group */}
      {processingItems.length > 0 && (
        <QueueGroup
          title={t('batch.processing_title')}
          count={processingItems.length}
          statusVariant="processing"
          defaultExpanded={true}
        >
          {processingItems.map(({ id }) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 2. Pending group */}
      {pendingItems.length > 0 && (
        <QueueGroup
          title={t('batch.queue_waiting')}
          count={pendingItems.length}
          statusVariant="pending"
          defaultExpanded={true}
        >
          {pendingItems.map(({ id }) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 3. Completed group */}
      {completeItems.length > 0 && (
        <QueueGroup
          title={t('batch.file_complete')}
          count={completeItems.length}
          statusVariant="complete"
          defaultExpanded={processingItems.length === 0 && pendingItems.length === 0}
        >
          {completeItems.map(({ id }) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 4. Failed group */}
      {failedItems.length > 0 && (
        <QueueGroup
          title={t('batch.file_failed')}
          count={failedItems.length}
          statusVariant="failed"
          defaultExpanded={true}
          action={
            <button
              className="btn btn-secondary-soft btn-xs queue-group-retry-all-btn"
              onClick={retryAllFailed}
              aria-label={t('batch.retry_all')}
            >
              <RestoreIcon width={11} height={11} />
              <span>{t('batch.retry_all')}</span>
            </button>
          }
        >
          {failedItems.map(({ id }) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}
    </div>
  );
}
