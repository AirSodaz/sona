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

  const queueItems = useBatchQueueStore((state) => state.queueItems);
  const totalCount = queueItems.length;

  const isLiveActive = ingestionFocus === 'live';
  const isBatchActive = ingestionFocus === 'batch';
  const activeIndex = isLiveActive ? 0 : 1;

  const handleTabKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      setIngestionFocus('live');
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      setIngestionFocus('batch');
    }
  };

  return (
    <div
      className={`create-workspace-shell state-${ingestionFocus} ${className}`}
      data-testid="create-workspace-shell"
    >
      <header
        className="create-workspace-header"
        aria-label={t('panel.mode_selection', { defaultValue: 'Mode Selection' })}
      >
        <div className="create-tab-navigation" role="tablist" onKeyDown={handleTabKeyDown}>
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
            className={`create-tab-button ${isLiveActive ? 'active' : ''}`}
            data-testid="create-tab-live"
            aria-selected={isLiveActive}
            onClick={() => setIngestionFocus('live')}
          >
            <Mic size={16} aria-hidden="true" />
            <span>{t('panel.live_record', { defaultValue: 'Live Record' })}</span>
            {isRecording && !isLiveActive ? (
              <span className="create-tab-badge live-recording" aria-hidden="true" />
            ) : null}
          </button>

          <button
            type="button"
            role="tab"
            className={`create-tab-button ${isBatchActive ? 'active' : ''}`}
            data-testid="create-tab-batch"
            aria-selected={isBatchActive}
            onClick={() => setIngestionFocus('batch')}
          >
            <Folder size={16} aria-hidden="true" />
            <span>{t('panel.batch_import', { defaultValue: 'Batch Import' })}</span>
            {totalCount > 0 && !isBatchActive ? (
              <span className="create-tab-badge batch-count">{totalCount}</span>
            ) : null}
          </button>
        </div>
      </header>

      <div className="create-workspace-body">
        <div
          className="create-pane create-pane-live"
          data-testid="pane-live"
          style={{ display: isLiveActive ? undefined : 'none' }}
        >
          <LiveRecord />
        </div>
        <div
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
