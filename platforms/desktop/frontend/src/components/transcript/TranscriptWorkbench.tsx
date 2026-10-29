import {
  Tv as EmbyIcon,
  Loader2,
  Users as SpeakerReviewIcon,
  History as VersionHistoryIcon,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { generateAiTitle } from '../../services/aiRenameService';
import { syncEmbySubtitles } from '../../services/emby/embyService';
import { useDialogStore } from '../../stores/dialogStore';
import { useEffectiveConfigStore } from '../../stores/effectiveConfigStore';
import { useEmbyHistoryStore } from '../../stores/embyHistoryStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { isHistoryItemDraft } from '../../types/history';
import { AudioPlayer } from '../AudioPlayer';
import { ErrorBoundary } from '../ErrorBoundary';
import { ExportButton } from '../ExportButton';
import {
  CloseIcon,
  CodeIcon,
  EditIcon,
  FileTextIcon,
  FolderIcon,
  MicIcon,
  SummaryIcon,
} from '../Icons';
import { PolishButton } from '../PolishButton';
import { RenameModal } from '../RenameModal';
import { TranslateButton } from '../TranslateButton';
import { TranscriptEditor } from './TranscriptEditor';
import { TranscriptSpeakerReviewPanel } from './TranscriptSpeakerReviewPanel';
import { TranscriptSummaryPanel } from './TranscriptSummaryPanel';
import { TranscriptVersionPanel } from './TranscriptVersionPanel';

interface TranscriptWorkbenchProps {
  /** Callback when the user clicks the close button. */
  onClose: () => void;
  /** Optional title to display. If not provided, will use the active transcript session title. */
  title?: string;
  /** Optional default type for the icon if the item has no icon set. */
  defaultIconType?: 'recording' | 'batch';
}

/**
 * Renders an icon based on the icon string or fallback to mode default
 */
function renderHeaderIcon(icon: string | null, defaultType: string): React.ReactNode {
  if (icon) {
    if (icon.startsWith('system:')) {
      const iconName = icon.replace('system:', '');
      switch (iconName) {
        case 'mic':
          return <MicIcon />;
        case 'file':
          return <FileTextIcon />;
        case 'folder':
          return <FolderIcon />;
        case 'code':
          return <CodeIcon />;
        default:
          break;
      }
    } else {
      return <span style={{ fontSize: '1.125rem', lineHeight: 1 }}>{icon}</span>;
    }
  }

  return defaultType === 'batch' ? <FileTextIcon /> : <MicIcon />;
}

/**
 * A unified workbench for transcript editing.
 * Combines the editor, audio player, AI summary access, and standard header.
 */
export function TranscriptWorkbench({
  onClose,
  title: propsTitle,
  defaultIconType,
}: TranscriptWorkbenchProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [isSummaryOpen, setIsSummaryOpen] = useState(false);
  const [isSpeakerReviewOpen, setIsSpeakerReviewOpen] = useState(false);
  const [isVersionPanelOpen, setIsVersionPanelOpen] = useState(false);
  const [isRenameModalOpen, setIsRenameModalOpen] = useState(false);

  // Store state
  const segments = useTranscriptSessionStore((state) => state.segments);
  const audioUrl = useTranscriptPlaybackStore((state) => state.audioUrl);
  const config = useEffectiveConfigStore((state) => state.config);
  const storeTitle = useTranscriptSessionStore((state) => state.title);
  const storeIcon = useTranscriptSessionStore((state) => state.icon);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const setTitle = useTranscriptSessionStore((state) => state.setTitle);
  const setIcon = useTranscriptSessionStore((state) => state.setIcon);
  const mode = useTranscriptRuntimeStore((state) => state.mode);
  const ingestionFocus = useTranscriptRuntimeStore((state) => state.ingestionFocus);

  const updateHistoryMeta = useHistoryStore((state) => state.updateItemMeta);
  const currentHistoryItem = useHistoryStore((state) =>
    sourceHistoryId ? state.items.find((item) => item.id === sourceHistoryId) || null : null
  );
  const embyContext = useEmbyHistoryStore((state) =>
    sourceHistoryId ? state.getContext(sourceHistoryId) : undefined
  );
  const [isSyncingEmby, setIsSyncingEmby] = useState(false);
  const [embySyncSuccess, setEmbySyncSuccess] = useState(false);
  const syncSuccessTimerRef = useRef<number | null>(null);

  useEffect(() => {
    void sourceHistoryId;
    setEmbySyncSuccess(false);
    setIsSyncingEmby(false);
    clearTimeout(syncSuccessTimerRef.current ?? undefined);
    syncSuccessTimerRef.current = null;
    return () => {
      clearTimeout(syncSuccessTimerRef.current ?? undefined);
      syncSuccessTimerRef.current = null;
    };
  }, [sourceHistoryId]);

  const handleResyncEmby = async () => {
    if (!embyContext || isSyncingEmby || segments.length === 0) return;
    setIsSyncingEmby(true);
    setEmbySyncSuccess(false);
    try {
      await syncEmbySubtitles(segments, embyContext);
      setEmbySyncSuccess(true);
      clearTimeout(syncSuccessTimerRef.current ?? undefined);
      syncSuccessTimerRef.current = window.setTimeout(() => {
        setEmbySyncSuccess(false);
        syncSuccessTimerRef.current = null;
      }, 3000);
    } catch (err: unknown) {
      console.error('Failed to resync to Emby:', err);
      void useDialogStore.getState().showError({
        code: 'emby.resync_failed',
        messageKey: 'errors.common.operation_failed',
        cause: err,
      });
    } finally {
      setIsSyncingEmby(false);
    }
  };

  useEscapeKey(
    (event) => {
      event.preventDefault();
      onClose();
    },
    {
      enabled: !isRecording,
      checkTopMost: true,
    }
  );

  const hasSegments = segments.length > 0;

  // Summary button logic
  const summaryEnabled = config.summaryEnabled ?? true;
  const showSummaryButton = summaryEnabled && hasSegments;
  const showVersionButton = Boolean(
    hasSegments &&
      sourceHistoryId &&
      sourceHistoryId !== 'current' &&
      (!currentHistoryItem || !isHistoryItemDraft(currentHistoryItem))
  );

  const historyDefaultIconType =
    currentHistoryItem?.type === 'batch'
      ? 'batch'
      : currentHistoryItem?.type === 'recording'
        ? 'recording'
        : undefined;
  const isBatchMode = mode === 'create' && ingestionFocus === 'batch';
  const displayIconType =
    defaultIconType || historyDefaultIconType || (isBatchMode ? 'batch' : 'recording');

  // Determine display title
  const fallbackTitle =
    displayIconType === 'batch'
      ? t('panel.batch_import', { defaultValue: 'Batch Import' })
      : t('panel.live_record', { defaultValue: 'Live Record' });
  const displayTitle = propsTitle || storeTitle || fallbackTitle;
  const isManualHeaderActionsDisabled = isRecording;
  const isRenameModalVisible = isRenameModalOpen && !isManualHeaderActionsDisabled;

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
          <div className="projects-detail-header-primary">
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                color: 'var(--color-text-secondary)',
              }}
            >
              {renderHeaderIcon(storeIcon, displayIconType)}
            </div>
            <h4
              style={{
                margin: 0,
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
              }}
              title={displayTitle}
            >
              {displayTitle}
            </h4>
            {embyContext && (
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  fontSize: '11px',
                  fontWeight: 500,
                  padding: '2px 8px',
                  borderRadius: '9999px',
                  background: 'rgba(82, 196, 26, 0.12)',
                  color: 'var(--color-success, #52c41a)',
                  border: '1px solid rgba(82, 196, 26, 0.25)',
                  whiteSpace: 'nowrap',
                  marginLeft: '4px',
                }}
                title={`Emby: ${embyContext.itemName}`}
              >
                <EmbyIcon size={12} />
                <span>Emby</span>
              </span>
            )}
            <button
              className="btn btn-icon btn-sm"
              type="button"
              onClick={() => {
                if (isManualHeaderActionsDisabled) {
                  return;
                }
                setIsRenameModalOpen(true);
              }}
              aria-label={t('common.rename', { defaultValue: 'Rename' })}
              disabled={isManualHeaderActionsDisabled}
              data-tooltip={t('common.rename', { defaultValue: 'Rename' })}
              data-tooltip-pos="bottom"
            >
              <EditIcon />
            </button>
            {showSummaryButton && (
              <button
                className="btn btn-icon btn-sm"
                onClick={() => setIsSummaryOpen(true)}
                aria-label={t('summary.title')}
                data-tooltip={t('summary.title')}
                data-tooltip-pos="bottom"
              >
                <SummaryIcon />
              </button>
            )}
            {hasSegments && (
              <button
                className="btn btn-icon btn-sm"
                type="button"
                onClick={() => setIsSpeakerReviewOpen(true)}
                aria-label={t('editor.speaker_review_title', { defaultValue: 'Speaker Review' })}
                data-tooltip={t('editor.speaker_review_title', { defaultValue: 'Speaker Review' })}
                data-tooltip-pos="bottom"
              >
                <SpeakerReviewIcon size={16} />
              </button>
            )}
            {showVersionButton && (
              <button
                className="btn btn-icon btn-sm"
                type="button"
                onClick={() => setIsVersionPanelOpen(true)}
                aria-label={t('versions.title', { defaultValue: 'Versions' })}
                data-tooltip={t('versions.title', { defaultValue: 'Versions' })}
                data-tooltip-pos="bottom"
              >
                <VersionHistoryIcon size={16} />
              </button>
            )}
          </div>
          <div className="projects-detail-header-actions">
            {embyContext && (
              <button
                type="button"
                className={`btn btn-sm ${embySyncSuccess ? 'btn-success' : 'btn-secondary'}`}
                onClick={handleResyncEmby}
                disabled={isSyncingEmby || isManualHeaderActionsDisabled || segments.length === 0}
                data-tooltip={t('emby.resync_tooltip', {
                  defaultValue: 'Re-sync subtitles to Emby server',
                })}
                data-tooltip-pos="bottom"
              >
                {isSyncingEmby ? <Loader2 size={14} className="spin" /> : <EmbyIcon size={14} />}
                <span>
                  {isSyncingEmby
                    ? t('emby.syncing', { defaultValue: 'Syncing...' })
                    : embySyncSuccess
                      ? t('emby.synced', { defaultValue: 'Synced' })
                      : t('emby.resync', { defaultValue: 'Re-sync to Emby' })}
                </span>
              </button>
            )}
            <PolishButton className="projects-detail-header-action" />
            <TranslateButton className="projects-detail-header-action" />
            <ExportButton className="projects-detail-header-action" />
            <button
              type="button"
              className="btn btn-icon"
              onClick={() => {
                if (isManualHeaderActionsDisabled) {
                  return;
                }
                onClose();
              }}
              aria-label={t('common.close', { defaultValue: 'Close' })}
              data-tooltip={t('common.close', { defaultValue: 'Close' })}
              data-tooltip-pos="bottom-left"
              disabled={isManualHeaderActionsDisabled}
            >
              <CloseIcon />
            </button>
          </div>
        </div>
      )}

      <div className="panel-content">
        <ErrorBoundary>
          <TranscriptEditor />
        </ErrorBoundary>
      </div>

      {audioUrl && <AudioPlayer />}

      <TranscriptSummaryPanel isOpen={isSummaryOpen} onClose={() => setIsSummaryOpen(false)} />

      <TranscriptSpeakerReviewPanel
        isOpen={isSpeakerReviewOpen}
        onClose={() => setIsSpeakerReviewOpen(false)}
      />

      <TranscriptVersionPanel
        isOpen={isVersionPanelOpen}
        historyId={sourceHistoryId}
        onClose={() => setIsVersionPanelOpen(false)}
      />

      <RenameModal
        isOpen={isRenameModalVisible}
        onClose={() => setIsRenameModalOpen(false)}
        initialTitle={displayTitle}
        initialIcon={storeIcon || undefined}
        defaultType={displayIconType === 'batch' ? 'batch' : 'recording'}
        onRename={handlePerformRename}
        onAiAction={async () => {
          return await generateAiTitle(segments);
        }}
      />
    </>
  );
}

export default TranscriptWorkbench;
