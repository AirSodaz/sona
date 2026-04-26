import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { ErrorBoundary } from './ErrorBoundary';
import { TranscriptEditor } from './TranscriptEditor';
import { AudioPlayer } from './AudioPlayer';
import { TranscriptSummaryPanel } from './TranscriptSummaryPanel';
import { RenameModal } from './RenameModal';
import { CloseIcon, SummaryIcon, EditIcon, MicIcon, FileTextIcon, FolderIcon, CodeIcon } from './Icons';
import { isSummaryLlmConfigComplete } from '../services/llmConfig';
import { generateAiTitle } from '../services/aiRenameService';

interface TranscriptWorkbenchProps {
  /** Callback when the user clicks the close button. */
  onClose: () => void;
  /** Optional title to display. If not provided, will use the one from transcriptStore. */
  title?: string;
}

/**
 * Renders an icon based on the icon string or fallback to mode default
 */
function renderHeaderIcon(icon: string | null, mode: string): React.ReactNode {
  if (icon) {
    if (icon.startsWith('system:')) {
      const iconName = icon.replace('system:', '');
      switch (iconName) {
        case 'mic': return <MicIcon />;
        case 'file': return <FileTextIcon />;
        case 'folder': return <FolderIcon />;
        case 'code': return <CodeIcon />;
        default: break;
      }
    } else {
      return <span style={{ fontSize: '1.125rem', lineHeight: 1 }}>{icon}</span>;
    }
  }

  return mode === 'batch' ? <FileTextIcon /> : <MicIcon />;
}

/**
 * A unified workbench for transcript editing.
 * Combines the editor, audio player, AI summary access, and standard header.
 */
export function TranscriptWorkbench({ onClose, title: propsTitle }: TranscriptWorkbenchProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [isSummaryOpen, setIsSummaryOpen] = useState(false);
  const [isRenameModalOpen, setIsRenameModalOpen] = useState(false);

  // Store state
  const segments = useTranscriptStore((state) => state.segments);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  const config = useTranscriptStore((state) => state.config);
  const storeTitle = useTranscriptStore((state) => state.title);
  const storeIcon = useTranscriptStore((state) => state.icon);
  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const setTitle = useTranscriptStore((state) => state.setTitle);
  const setIcon = useTranscriptStore((state) => state.setIcon);
  const mode = useTranscriptStore((state) => state.mode);
  
  const updateHistoryMeta = useHistoryStore((state) => state.updateItemMeta);

  const hasSegments = segments.length > 0;
  
  // Summary button logic
  const summaryEnabled = config.summaryEnabled ?? true;
  const summaryConfigComplete = isSummaryLlmConfigComplete(config);
  const showSummaryButton = summaryEnabled && summaryConfigComplete && hasSegments;

  // Determine display title
  const displayTitle = propsTitle || storeTitle || (mode === 'live' ? t('panel.live_record') : t('panel.batch_import'));

  const handlePerformRename = async (newTitle: string, newIcon?: string) => {
    setTitle(newTitle.trim());
    setIcon(newIcon || null);
    if (sourceHistoryId) {
      await updateHistoryMeta(sourceHistoryId, { title: newTitle.trim(), icon: newIcon });
    }
    setIsRenameModalOpen(false);
  };

  return (
    <>
      {hasSegments && (
        <div className="projects-detail-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', minWidth: 0, flex: 1 }}>
            <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-secondary)' }}>
              {renderHeaderIcon(storeIcon, mode)}
            </div>
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
            <button
              className="btn btn-icon btn-sm"
              onClick={() => setIsRenameModalOpen(true)}
              data-tooltip={t('common.rename', { defaultValue: 'Rename' })}
              data-tooltip-pos="bottom"
            >
              <EditIcon />
            </button>
            {showSummaryButton && (
              <button
                className="btn btn-icon btn-sm"
                onClick={() => setIsSummaryOpen(true)}
                data-tooltip={t('summary.title')}
                data-tooltip-pos="bottom"
              >
                <SummaryIcon />
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

      <RenameModal
        isOpen={isRenameModalOpen}
        onClose={() => setIsRenameModalOpen(false)}
        initialTitle={displayTitle}
        initialIcon={storeIcon || undefined}
        defaultType={mode === 'batch' ? 'batch' : 'recording'}
        onRename={handlePerformRename}
        onAiAction={async () => {
          return await generateAiTitle(segments);
        }}
      />
    </>
  );
}

export default TranscriptWorkbench;
