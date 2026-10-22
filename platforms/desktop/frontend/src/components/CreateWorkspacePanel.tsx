import { Folder, Mic } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { BatchImport } from './BatchImport';
import { LiveRecord } from './LiveRecord';
import '../styles/create-workspace-panel.css';

interface CreateWorkspacePanelProps {
  className?: string;
}

export function CreateWorkspacePanel({
  className = '',
}: CreateWorkspacePanelProps): React.JSX.Element {
  const { t } = useTranslation();

  const ingestionFocus = useTranscriptRuntimeStore((state) => state.ingestionFocus);
  const setIngestionFocus = useTranscriptStore((state) => state.setIngestionFocus);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const isPaused = useTranscriptRuntimeStore((state) => state.isPaused);

  const queueItems = useBatchQueueStore((state) => state.queueItems);

  // Calculate overall batch progress
  const totalCount = queueItems.length;
  let progressSum = 0;
  queueItems.forEach((item) => {
    if (item.status === 'complete') {
      progressSum += 100;
    } else if (item.status === 'processing') {
      progressSum += item.progress;
    }
  });
  const overallProgress =
    totalCount > 0 ? Math.min(100, Math.round((progressSum / (totalCount * 100)) * 100)) : 0;

  const isLiveActive = ingestionFocus === 'live';
  const isBatchActive = ingestionFocus === 'batch';

  return (
    <div
      className={`create-workspace-shell state-${ingestionFocus} ${className}`}
      data-testid="create-workspace-shell"
    >
      {/* Upper Deck: Live Record */}
      <section
        className={`deck-deck deck-live ${isLiveActive ? 'is-expanded' : 'is-collapsed'}`}
        aria-label={t('panel.live_record')}
      >
        {isLiveActive ? (
          <div className="deck-expanded-header" data-testid="deck-live-header">
            <div className="deck-left-info">
              {isRecording ? (
                <span className="deck-live-dot recording" aria-hidden="true" />
              ) : (
                <Mic size={16} aria-hidden="true" />
              )}
              <span className="deck-title">{t('panel.live_record')}</span>
            </div>
            <div className="deck-right-info" />
          </div>
        ) : (
          <button
            type="button"
            className="deck-collapsed-bar"
            data-testid="deck-live-collapsed-bar"
            onClick={() => setIngestionFocus('live')}
            aria-label={t('panel.live_record')}
          >
            <div className="deck-left-info">
              {isRecording ? (
                <span className="deck-live-dot recording" aria-hidden="true" />
              ) : (
                <Mic size={16} aria-hidden="true" />
              )}
              <span className="deck-title">{t('panel.live_record')}</span>
            </div>
            <div className="deck-right-info">
              {isRecording ? (
                <span className="deck-live-recording-badge">
                  <span className="deck-live-dot recording" aria-hidden="true" />
                  <span>
                    {isPaused
                      ? t('live.paused', { defaultValue: '已暂停' })
                      : t('live.recording', { defaultValue: '录音中' })}
                  </span>
                </span>
              ) : null}
            </div>
          </button>
        )}
        <div
          className="deck-body deck-body-live"
          data-testid="deck-body-live"
          style={{ display: isLiveActive ? undefined : 'none' }}
        >
          <LiveRecord />
        </div>
      </section>

      {/* Lower Deck: Batch Import */}
      <section
        className={`deck-deck deck-batch ${isBatchActive ? 'is-expanded' : 'is-collapsed'}`}
        aria-label={t('panel.batch_import')}
      >
        {isBatchActive ? (
          <div className="deck-expanded-header" data-testid="deck-batch-header">
            <div className="deck-left-info">
              <Folder size={16} aria-hidden="true" />
              <span className="deck-title">{t('panel.batch_import')}</span>
            </div>
            <div className="deck-right-info" />
          </div>
        ) : (
          <button
            type="button"
            className="deck-collapsed-bar"
            data-testid="deck-batch-collapsed-bar"
            onClick={() => setIngestionFocus('batch')}
            aria-label={t('panel.batch_import')}
          >
            <div className="deck-left-info">
              <Folder size={16} aria-hidden="true" />
              <span className="deck-title">{t('panel.batch_import')}</span>
            </div>
            <div className="deck-right-info">
              {totalCount > 0 ? (
                <>
                  <span className="deck-compact-summary">
                    {t('batch.queue_summary_compact', {
                      defaultValue: `${totalCount} 个文件 · ${overallProgress}%`,
                      count: totalCount,
                      progress: overallProgress,
                    })}
                  </span>
                  <div className="deck-mini-progress" aria-hidden="true">
                    <div
                      className="deck-mini-progress-fill"
                      style={{ width: `${overallProgress}%` }}
                    />
                  </div>
                </>
              ) : null}
            </div>
          </button>
        )}
        <div
          className="deck-body deck-body-batch"
          data-testid="deck-body-batch"
          style={{ display: isBatchActive ? undefined : 'none' }}
        >
          <BatchImport />
        </div>
      </section>
    </div>
  );
}
