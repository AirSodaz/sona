import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
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
 * - Waiting (pending items)
 * - Completed (collapsible)
 * - Failed (with batch retry controls)
 */
export function QueueGroupedList({
  className = '',
}: QueueGroupedListProps): React.JSX.Element | null {
  const { t } = useTranslation();

  const processingIds = useBatchQueueStore(
    useShallow((state) =>
      state.queueItems.filter((i) => i.status === 'processing').map((i) => i.id)
    )
  );
  const pendingIds = useBatchQueueStore(
    useShallow((state) => state.queueItems.filter((i) => i.status === 'pending').map((i) => i.id))
  );
  const completeIds = useBatchQueueStore(
    useShallow((state) => state.queueItems.filter((i) => i.status === 'complete').map((i) => i.id))
  );
  const failedIds = useBatchQueueStore(
    useShallow((state) =>
      state.queueItems
        .filter((i) => i.status === 'error' || i.status === 'cancelled')
        .map((i) => i.id)
    )
  );
  const totalCount = useBatchQueueStore((state) => state.queueItems.length);
  const retryAllFailed = useBatchQueueStore((state) => state.retryAllFailed);

  if (totalCount === 0) {
    return null;
  }
  return (
    <div
      className={`queue-grouped-list ${className}`}
      role="list"
      aria-label={t('batch.queue_title', { count: totalCount })}
    >
      {/* 1. Processing group */}
      {processingIds.length > 0 && (
        <QueueGroup
          title={t('batch.group_processing')}
          count={processingIds.length}
          statusVariant="processing"
          defaultExpanded={true}
        >
          {processingIds.map((id) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 2. Pending group */}
      {pendingIds.length > 0 && (
        <QueueGroup
          title={t('batch.group_pending')}
          count={pendingIds.length}
          statusVariant="pending"
          defaultExpanded={true}
        >
          {pendingIds.map((id) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 3. Completed group */}
      {completeIds.length > 0 && (
        <QueueGroup
          title={t('batch.group_complete')}
          count={completeIds.length}
          statusVariant="complete"
          defaultExpanded={processingIds.length === 0 && pendingIds.length === 0}
        >
          {completeIds.map((id) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}

      {/* 4. Failed group */}
      {failedIds.length > 0 && (
        <QueueGroup
          title={t('batch.group_failed')}
          count={failedIds.length}
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
          {failedIds.map((id) => (
            <QueueItemContainer key={id} id={id} t={t} />
          ))}
        </QueueGroup>
      )}
    </div>
  );
}
