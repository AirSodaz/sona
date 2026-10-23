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

  const totalCount = useBatchQueueStore((state) => state.queueItems.length);
  const isQueueProcessing = useBatchQueueStore((state) => state.isQueueProcessing);
  const hasProcessingItem = useBatchQueueStore((state) =>
    state.queueItems.some((item) => item.status === 'processing')
  );
  const isBatchProcessing = isQueueProcessing || hasProcessingItem;
  const isLiveActive = ingestionFocus === 'live';
  const isBatchActive = ingestionFocus === 'batch';
  const activeIndex = isLiveActive ? 0 : 1;

  const handleTabKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || e.key === 'Home') {
      e.preventDefault();
      setIngestionFocus('live');
      document.getElementById('create-tab-live')?.focus();
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || e.key === 'End') {
      e.preventDefault();
      setIngestionFocus('batch');
      document.getElementById('create-tab-batch')?.focus();
    }
  };

  return (
    <div
      className={`create-workspace-shell state-${ingestionFocus} ${className}`}
      data-testid="create-workspace-shell"
    >
      <header className="create-workspace-header">
        <div
          className="create-tab-navigation"
          role="tablist"
          aria-label={t('panel.mode_selection', { defaultValue: 'Mode Selection' })}
          onKeyDown={handleTabKeyDown}
        >
          <div
            className="create-tab-slider"
            style={{
              transform: `translateX(calc(${activeIndex} * (100% + 4px)))`,
            }}
            aria-hidden="true"
          />
          <button
            type="button"
            role="tab"
            id="create-tab-live"
            aria-controls="pane-live"
            tabIndex={isLiveActive ? 0 : -1}
            className={`create-tab-button ${isLiveActive ? 'active' : ''}`}
            data-testid="create-tab-live"
            aria-selected={isLiveActive}
            onClick={() => setIngestionFocus('live')}
          >
            <Mic size={16} aria-hidden="true" />
            <span>{t('panel.live_record', { defaultValue: 'Live Record' })}</span>
            {isRecording && !isLiveActive ? (
              <>
                <span className="create-tab-badge live-recording" aria-hidden="true" />
                <span className="sr-only">
                  ({t('live.recording', { defaultValue: 'Recording' })})
                </span>
              </>
            ) : null}
          </button>

          <button
            type="button"
            role="tab"
            id="create-tab-batch"
            aria-controls="pane-batch"
            tabIndex={isBatchActive ? 0 : -1}
            className={`create-tab-button ${isBatchActive ? 'active' : ''}`}
            data-testid="create-tab-batch"
            aria-selected={isBatchActive}
            onClick={() => setIngestionFocus('batch')}
          >
            <Folder size={16} aria-hidden="true" />
            <span>{t('panel.batch_import', { defaultValue: 'Batch Import' })}</span>
            {isBatchProcessing && !isBatchActive ? (
              <>
                <span className="create-tab-badge batch-processing" aria-hidden="true" />
                <span className="sr-only">
                  ({t('batch.processing', { defaultValue: 'Processing' })})
                </span>
              </>
            ) : totalCount > 0 && !isBatchActive ? (
              <span className="create-tab-badge batch-count">
                <span aria-hidden="true">{totalCount}</span>
                <span className="sr-only">
                  {' '}
                  ({totalCount} {t('batch.items', { defaultValue: 'items' })})
                </span>
              </span>
            ) : null}
          </button>
        </div>
      </header>

      <div className="create-workspace-body">
        <div
          id="pane-live"
          role="tabpanel"
          aria-labelledby="create-tab-live"
          tabIndex={0}
          className="create-pane create-pane-live"
          data-testid="pane-live"
          style={{ display: isLiveActive ? undefined : 'none' }}
        >
          <LiveRecord />
        </div>
        <div
          id="pane-batch"
          role="tabpanel"
          aria-labelledby="create-tab-batch"
          tabIndex={0}
          className="create-pane create-pane-batch"
          data-testid="pane-batch"
          style={{ display: isBatchActive ? undefined : 'none' }}
        >
          <BatchImport />
        </div>
      </div>
    </div>
  );
}
