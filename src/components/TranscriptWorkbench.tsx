import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { ErrorBoundary } from './ErrorBoundary';
import { TranscriptEditor } from './TranscriptEditor';
import { AudioPlayer } from './AudioPlayer';
import { TranscriptSummaryPanel } from './TranscriptSummaryPanel';
import { CloseIcon, SparklesIcon } from './Icons';
import { isSummaryLlmConfigComplete } from '../services/llmConfig';

interface TranscriptWorkbenchProps {
  /** Callback when the user clicks the close button. */
  onClose: () => void;
  /** Optional title to display. If not provided, will use the one from transcriptStore. */
  title?: string;
}

/**
 * A unified workbench for transcript editing.
 * Combines the editor, audio player, AI summary access, and standard header.
 */
export function TranscriptWorkbench({ onClose, title: propsTitle }: TranscriptWorkbenchProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [isSummaryOpen, setIsSummaryOpen] = useState(false);

  // Store state
  const segments = useTranscriptStore((state) => state.segments);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  const config = useTranscriptStore((state) => state.config);
  const storeTitle = useTranscriptStore((state) => state.title);
  const mode = useTranscriptStore((state) => state.mode);

  const hasSegments = segments.length > 0;
  
  // Summary button logic
  const summaryEnabled = config.summaryEnabled ?? true;
  const summaryConfigComplete = isSummaryLlmConfigComplete(config);
  const showSummaryButton = summaryEnabled && summaryConfigComplete && hasSegments;

  // Determine display title
  const displayTitle = propsTitle || storeTitle || (mode === 'live' ? t('panel.live_record') : t('panel.batch_import'));

  return (
    <>
      {hasSegments && (
        <div className="projects-detail-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', minWidth: 0, flex: 1 }}>
            <h4 
              style={{ 
                margin: 0, 
                whiteSpace: 'nowrap', 
                overflow: 'hidden', 
                textOverflow: 'ellipsis' 
              }} 
              title={displayTitle}
            >
              {displayTitle}
            </h4>
            {showSummaryButton && (
              <button
                className="btn btn-icon btn-sm"
                onClick={() => setIsSummaryOpen(true)}
                data-tooltip={t('summary.title')}
                data-tooltip-pos="bottom"
              >
                <SparklesIcon />
              </button>
            )}
          </div>
          <button
            type="button"
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close', { defaultValue: 'Close' })}
          >
            <CloseIcon />
          </button>
        </div>
      )}
      
      <div className="panel-content">
        <ErrorBoundary>
          <TranscriptEditor />
        </ErrorBoundary>
      </div>
      
      {audioUrl && <AudioPlayer />}

      <TranscriptSummaryPanel 
        isOpen={isSummaryOpen} 
        onClose={() => setIsSummaryOpen(false)} 
      />
    </>
  );
}

export default TranscriptWorkbench;
