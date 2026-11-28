import {
  Check,
  CloudUpload,
  Users as SpeakerReviewIcon,
  History as VersionHistoryIcon,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useBoundSessionId } from '../../contexts/TranscriptSessionContext';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { generateAiTitle } from '../../services/aiRenameService';
import { resyncMediaServerSubtitles } from '../../services/mediaServer/mediaServerService';
import {
  selectIsDocumentEditorLocked,
  useDocumentMutationLockStore,
} from '../../services/task/documentMutationCoordinator';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useEffectiveConfigStore } from '../../stores/effectiveConfigStore';
import { useEmbyHistoryStore } from '../../stores/embyHistoryStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { isHistoryItemDraft } from '../../types/history';
import { logger } from '../../utils/logger';
import { AudioPlayer } from '../AudioPlayer';
import { ErrorBoundary } from '../ErrorBoundary';
import { ExportButton } from '../ExportButton';
import {
  getMediaServerIconId,
  getMediaServerIconMeta,
  getMediaServerLabel,
  renderSystemIcon,
} from '../IconPicker';
import { CloseIcon, EditIcon, FileTextIcon, MicIcon, SummaryIcon } from '../Icons';
import { PolishButton } from '../PolishButton';
import { RenameModal } from '../RenameModal';
import { TranslateButton } from '../TranslateButton';
import { IconButton } from '../ui/Button';
import { Tooltip } from '../ui/Tooltip';
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
  /** Whether this workbench's shell is visible. Hidden shells keep the editor mounted but
   * must not own global audio playback or keyboard shortcuts. Defaults to true. */
  isActive?: boolean;
}

/**
 * Renders an icon based on the icon string or fallback to mode default
 */
function renderHeaderIcon(icon: string | null, defaultType: string): React.ReactNode {
  if (icon) {
    if (icon.startsWith('system:')) {
      const resolved = renderSystemIcon(icon);
      if (resolved) {
        return resolved;
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
  isActive = true,
}: TranscriptWorkbenchProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const boundSessionId = useBoundSessionId();
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
  const recordingSessionId = useTranscriptStore((state) => state.recordingSessionId);
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
  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    void sourceHistoryId;
    setIsSyncingEmby(false);
    setEmbySyncSuccess(false);
    clearTimeout(syncSuccessTimerRef.current ?? undefined);
    syncSuccessTimerRef.current = null;
    return () => {
      clearTimeout(syncSuccessTimerRef.current ?? undefined);
      syncSuccessTimerRef.current = null;
    };
  }, [sourceHistoryId]);

  const handleResyncEmby = async () => {
    if (!embyContext || isSyncingEmby || segments.length === 0) return;
    const targetHistoryId = sourceHistoryId;
    setIsSyncingEmby(true);
    setEmbySyncSuccess(false);
    try {
      await resyncMediaServerSubtitles(segments, embyContext);
      if (
        !isMountedRef.current ||
        useTranscriptStore.getState().sessions[boundSessionId]?.sourceHistoryId !== targetHistoryId
      )
        return;
      setEmbySyncSuccess(true);
      clearTimeout(syncSuccessTimerRef.current ?? undefined);
      syncSuccessTimerRef.current = window.setTimeout(() => {
        if (!isMountedRef.current) return;
        setEmbySyncSuccess(false);
        syncSuccessTimerRef.current = null;
      }, 3000);
    } catch (err: unknown) {
      if (
        !isMountedRef.current ||
        useTranscriptStore.getState().sessions[boundSessionId]?.sourceHistoryId !== targetHistoryId
      )
        return;
      logger.error(
        `[TranscriptWorkbench] Failed to resync to ${getMediaServerLabel(embyContext.serverType)}:`,
        err
      );
      void useDialogStore.getState().showError({
        code: 'emby.resync_failed',
        messageKey: 'errors.common.operation_failed',
        cause: err,
      });
    } finally {
      if (
        isMountedRef.current &&
        useTranscriptStore.getState().sessions[boundSessionId]?.sourceHistoryId === targetHistoryId
      ) {
        setIsSyncingEmby(false);
      }
    }
  };

  const isCurrentSessionRecording = Boolean(
    isRecording &&
      (recordingSessionId
        ? boundSessionId === recordingSessionId
        : sourceHistoryId === 'current' || !sourceHistoryId)
  );
  const isDocumentLocked = useDocumentMutationLockStore((state) =>
    selectIsDocumentEditorLocked(state.locks, boundSessionId, sourceHistoryId)
  );
  const isBatchProcessing = useBatchQueueStore((state) =>
    state.queueItems.some(
      (item) =>
        item.status === 'processing' &&
        (item.id === boundSessionId ||
          (Boolean(sourceHistoryId) && item.historyId === sourceHistoryId))
    )
  );

  useEscapeKey(
    (event) => {
      event.preventDefault();
      onClose();
    },
    {
      enabled: isActive && !isCurrentSessionRecording,
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
  const isManualHeaderActionsDisabled =
    isCurrentSessionRecording || isDocumentLocked || isBatchProcessing;
  const isRenameModalVisible = isRenameModalOpen && !isManualHeaderActionsDisabled;
  const mediaServerLabel = embyContext ? getMediaServerLabel(embyContext.serverType) : null;
  const mediaIconMeta = getMediaServerIconMeta(storeIcon);
  const mediaServerResetIcon = embyContext
    ? getMediaServerIconId(embyContext.serverType)
    : mediaIconMeta
      ? (storeIcon ?? undefined)
      : undefined;
  const resyncActionLabel = !mediaServerLabel
    ? ''
    : isSyncingEmby
      ? t('emby.syncing', { defaultValue: 'Syncing...' })
      : embySyncSuccess
        ? t('emby.synced', { defaultValue: 'Synced' })
        : t('media_server.resync_tooltip', {
            server: mediaServerLabel,
            defaultValue: `Re-sync subtitles to ${mediaServerLabel} server`,
          });

  const handlePerformRename = async (newTitle: string, newIcon?: string) => {
    setTitle(newTitle.trim());
    setIcon(newIcon || null);
    if (sourceHistoryId) {
      await updateHistoryMeta(sourceHistoryId, { title: newTitle.trim(), icon: newIcon || null });
    }
    setIsRenameModalOpen(false);
  };

  return (
    <>
      {hasSegments && (
        <div className="projects-detail-header">
          <div className="projects-detail-header-primary">
            {mediaIconMeta ? (
              <Tooltip
                content={
                  embyContext
                    ? `${mediaIconMeta.label}: ${embyContext.itemName}`
                    : mediaIconMeta.label
                }
                placement="bottom"
              >
                <span
                  data-testid="media-server-header-icon"
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    flexShrink: 0,
                  }}
                >
                  {renderHeaderIcon(storeIcon, displayIconType)}
                </span>
              </Tooltip>
            ) : (
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  color: 'var(--color-text-secondary)',
                  flexShrink: 0,
                }}
              >
                {renderHeaderIcon(storeIcon, displayIconType)}
              </div>
            )}
            <Tooltip content={displayTitle} placement="bottom" onlyWhenTruncated>
              <h4
                style={{
                  margin: 0,
                  minWidth: 0,
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {displayTitle}
              </h4>
            </Tooltip>
            <IconButton
              tier="ghost"
              size="sm"
              icon={<EditIcon />}
              onClick={() => {
                if (isManualHeaderActionsDisabled) {
                  return;
                }
                setIsRenameModalOpen(true);
              }}
              aria-label={t('common.rename', { defaultValue: 'Rename' })}
              disabled={isManualHeaderActionsDisabled}
              tooltipPos="bottom"
            />
            {showSummaryButton && (
              <IconButton
                tier="ghost"
                size="sm"
                icon={<SummaryIcon />}
                onClick={() => setIsSummaryOpen(true)}
                aria-label={t('summary.title')}
                tooltipPos="bottom"
              />
            )}
            {hasSegments && (
              <IconButton
                tier="ghost"
                size="sm"
                icon={<SpeakerReviewIcon size={16} />}
                onClick={() => setIsSpeakerReviewOpen(true)}
                aria-label={t('editor.speaker_review_title', { defaultValue: 'Speaker Review' })}
                tooltipPos="bottom"
              />
            )}
            {showVersionButton && (
              <IconButton
                tier="ghost"
                size="sm"
                icon={<VersionHistoryIcon size={16} />}
                onClick={() => setIsVersionPanelOpen(true)}
                aria-label={t('versions.title', { defaultValue: 'Versions' })}
                tooltipPos="bottom"
              />
            )}
          </div>
          <div className="projects-detail-header-actions">
            {embyContext && (
              <IconButton
                tier="ghost"
                size="md"
                className="projects-detail-header-action"
                loading={isSyncingEmby}
                feedback={embySyncSuccess ? 'success' : 'idle'}
                icon={embySyncSuccess ? <Check size={16} /> : <CloudUpload size={16} />}
                onClick={handleResyncEmby}
                disabled={isManualHeaderActionsDisabled || segments.length === 0}
                aria-label={resyncActionLabel}
                tooltip={resyncActionLabel}
                tooltipPos="bottom"
              />
            )}
            <PolishButton className="projects-detail-header-action" />
            <TranslateButton className="projects-detail-header-action" />
            <ExportButton className="projects-detail-header-action" />
            <IconButton
              tier="ghost"
              size="md"
              icon={<CloseIcon />}
              onClick={() => {
                if (isManualHeaderActionsDisabled) {
                  return;
                }
                onClose();
              }}
              aria-label={t('common.close', { defaultValue: 'Close' })}
              tooltipPos="bottom-left"
              disabled={isManualHeaderActionsDisabled}
            />
          </div>
        </div>
      )}

      <div className="panel-content">
        <ErrorBoundary>
          <TranscriptEditor isActive={isActive} />
        </ErrorBoundary>
      </div>

      {isActive && audioUrl && <AudioPlayer isActive={isActive} />}

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
        resetIconValue={mediaServerResetIcon}
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
