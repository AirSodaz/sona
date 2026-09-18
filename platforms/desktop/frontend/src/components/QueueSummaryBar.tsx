import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { CompleteIcon, ErrorIcon, PauseIcon, PlayIcon, ProcessingIcon, UploadIcon } from './Icons';
import { QueueClearMenu } from './QueueClearMenu';

export interface QueueSummaryBarProps {
  onAddFiles: () => void;
  className?: string;
}

/**
 * Top summary panel for the batch transcription queue.
 * Displays overall queue progress, status pill, and batch actions (Start/Pause, Add, Clear).
 */
export function QueueSummaryBar({
  onAddFiles,
  className = '',
}: QueueSummaryBarProps): React.JSX.Element | null {
  const { t } = useTranslation();

  const queueItems = useBatchQueueStore((state) => state.queueItems);
  const isPaused = useBatchQueueStore((state) => state.isQueuePaused);
  const isProcessing = useBatchQueueStore((state) => state.isQueueProcessing);

  const clearQueue = useBatchQueueStore((state) => state.clearQueue);
  const clearCompleted = useBatchQueueStore((state) => state.clearCompleted);
  const pauseQueue = useBatchQueueStore((state) => state.pauseQueue);
  const resumeQueue = useBatchQueueStore((state) => state.resumeQueue);
  const processQueue = useBatchQueueStore((state) => state.processQueue);

  const totalCount = queueItems.length;
  if (totalCount === 0) {
    return null;
  }

  let completedCount = 0;
  let progressSum = 0;
  queueItems.forEach((item) => {
    if (item.status === 'complete') {
      completedCount += 1;
      progressSum += 100;
    } else if (item.status === 'processing') {
      progressSum += item.progress;
    }
  });

  const overallProgress =
    totalCount > 0 ? Math.min(100, Math.round((progressSum / (totalCount * 100)) * 100)) : 0;
  const errorCount = queueItems.filter(
    (item) => item.status === 'error' || item.status === 'cancelled'
  ).length;
  const isAllComplete = completedCount === totalCount && totalCount > 0;
  const isAllFailed = errorCount === totalCount && totalCount > 0 && !isProcessing;
  const hasPendingItems = queueItems.some((item) => item.status === 'pending');
  return (
    <div className={`queue-summary-bar ${className}`}>
      <div className="queue-summary-top">
        <div className="queue-summary-info">
          <span className="queue-summary-title">
            {t('batch.queue_title', { count: totalCount })}
          </span>

          <span
            className={`queue-summary-status-pill ${
              isProcessing
                ? 'status-processing'
                : isPaused
                  ? 'status-paused'
                  : isAllComplete
                    ? 'status-complete'
                    : errorCount > 0
                      ? 'status-error'
                      : 'status-idle'
            }`}
          >
            {isProcessing ? (
              <>
                <ProcessingIcon width={12} height={12} />
                <span>
                  {t('batch.transcribing')} ({completedCount}/{totalCount})
                </span>
              </>
            ) : isPaused ? (
              <>
                <PauseIcon width={12} height={12} />
                <span>{t('batch.pause_queue')}</span>
              </>
            ) : isAllComplete ? (
              <>
                <CompleteIcon width={12} height={12} />
                <span>{t('batch.file_complete')}</span>
              </>
            ) : errorCount > 0 ? (
              <>
                <ErrorIcon width={12} height={12} />
                <span>
                  {completedCount}/{totalCount} · {errorCount} {t('batch.file_failed')}
                </span>
              </>
            ) : (
              <span>
                {t('batch.summary_progress', { done: completedCount, total: totalCount })}
              </span>
            )}
          </span>
        </div>

        <span className="queue-summary-progress-text">{overallProgress}%</span>
      </div>

      <div
        className="queue-summary-progress-track"
        role="progressbar"
        aria-valuenow={overallProgress}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={t('batch.queue_title', { count: totalCount })}
      >
        <div
          className={`queue-summary-progress-fill ${
            isAllComplete ? 'fill-complete' : isAllFailed ? 'fill-error' : ''
          }`}
          style={{ width: isAllFailed ? '100%' : `${overallProgress}%` }}
        />
      </div>

      <div className="queue-summary-toolbar">
        <div className="queue-summary-toolbar-left">
          {/* Start / Pause / Resume control */}
          {isProcessing ? (
            <button
              className="btn btn-secondary-soft btn-sm queue-btn-action"
              onClick={pauseQueue}
              aria-label={t('batch.pause_queue')}
            >
              <PauseIcon width={13} height={13} />
              <span>{t('batch.pause_queue')}</span>
            </button>
          ) : isPaused ? (
            <button
              className="btn btn-primary btn-sm queue-btn-action"
              onClick={resumeQueue}
              aria-label={t('batch.resume_queue')}
            >
              <PlayIcon width={13} height={13} />
              <span>{t('batch.resume_queue')}</span>
            </button>
          ) : hasPendingItems && !isProcessing ? (
            <button
              className="btn btn-primary btn-sm queue-btn-action"
              onClick={() => void processQueue()}
              aria-label={t('batch.start_queue')}
            >
              <PlayIcon width={13} height={13} />
              <span>{t('batch.start_queue')}</span>
            </button>
          ) : null}

          {/* Add more files button */}
          <button
            className="btn btn-secondary-soft btn-sm queue-btn-action"
            onClick={onAddFiles}
            aria-label={t('batch.add_more_files')}
          >
            <UploadIcon width={13} height={13} />
            <span>{t('batch.add_more_files')}</span>
          </button>
        </div>

        {/* Clear queue dropdown menu */}
        <QueueClearMenu
          completedCount={completedCount}
          totalCount={totalCount}
          onClear={(scope) => {
            if (scope === 'completed') {
              clearCompleted();
            } else {
              clearQueue();
            }
          }}
        />
      </div>
    </div>
  );
}
