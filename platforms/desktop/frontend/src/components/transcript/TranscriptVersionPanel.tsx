import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckSquare,
  Clock3,
  FileDiff,
  Loader2,
  RotateCcw,
  Square,
  Undo2,
} from 'lucide-react';
import { useVersionPanel } from '../../hooks/useVersionPanel';
import type { TranscriptSegment } from '../../types/transcript';
import type {
  TranscriptDiffRow,
  TranscriptSnapshotReason,
} from '../../types/transcriptSnapshot';
import { PanelModal } from '../PanelModal';
import './TranscriptVersionPanel.css';

interface TranscriptVersionPanelProps {
  isOpen: boolean;
  historyId: string | null;
  onClose: () => void;
}

function formatSegmentTime(segment: TranscriptSegment | undefined): string {
  if (!segment) {
    return '';
  }

  const minutes = Math.floor(Math.max(segment.start, 0) / 60);
  const seconds = Math.floor(Math.max(segment.start, 0) % 60);
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

function getSegmentText(segment: TranscriptSegment | undefined): string {
  if (!segment) {
    return '';
  }

  const translation = segment.translation?.trim();
  return translation ? `${segment.text}\n${translation}` : segment.text;
}

function getReasonLabel(
  reason: TranscriptSnapshotReason,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  return t(`versions.reason.${reason}`);
}

function getStatusLabel(
  status: TranscriptDiffRow['status'],
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  return t(`versions.diff.${status}`);
}

export function TranscriptVersionPanel({
  isOpen,
  historyId,
  onClose,
}: TranscriptVersionPanelProps): React.JSX.Element | null {
  const { t } = useTranslation();
  
  const {
    snapshots,
    selectedSnapshotId,
    setSelectedSnapshotId,
    selectedRecord,
    diffRows,
    changedCount,
    changedRows,
    selectedRowIds,
    setSelectedRowIds,
    isLoading,
    isDiffLoading,
    isBusy,
    error,
    toggleRow,
    handleToggleAll,
    handleRestoreSelected,
    handleRestoreAll,
  } = useVersionPanel({ isOpen, historyId });

  if (!isOpen) {
    return null;
  }

  const allChangedRowsSelected = changedRows.length > 0 && selectedRowIds.size === changedRows.length;

  return (
    <PanelModal
      isOpen={isOpen}
      onClose={onClose}
      ariaLabelledby="transcript-version-title"
      size="settings"
      className="transcript-version-modal"
      overlayClassName="transcript-version-overlay"
      headerClassName="transcript-version-header"
      badgeClassName="transcript-version-badge"
      toolbarClassName="transcript-version-actions"
      metaClassName="transcript-version-meta-row"
      contentClassName="transcript-version-content"
      badge={(
        <>
          <FileDiff size={16} />
          <span>{t('versions.badge')}</span>
        </>
      )}
      title={<h2 id="transcript-version-title">{t('versions.title')}</h2>}
      description={t('versions.description')}
      headerActions={(
        <>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void handleRestoreSelected().then((success) => success && onClose())}
            disabled={isBusy || isDiffLoading || selectedRowIds.size === 0}
          >
            {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <Undo2 size={14} />}
            {t('versions.restore_selected')}
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void handleRestoreAll().then((success) => success && onClose())}
            disabled={isBusy || !selectedRecord}
          >
            <RotateCcw size={14} />
            {t('versions.restore_all')}
          </button>
        </>
      )}
      meta={(
        <>
          <span className="panel-modal-meta-label">{t('versions.snapshot_count')}</span>
          <span>{snapshots.length}</span>
          {selectedRecord ? (
            <>
              <span className="panel-modal-meta-label">{t('versions.changed_count')}</span>
              <span>{changedCount}</span>
            </>
          ) : null}
        </>
      )}
      errorBanner={error ? (
        <div className="transcript-version-error" role="alert">
          {error}
        </div>
      ) : null}
    >
      <aside className="transcript-version-sidebar" aria-label={t('versions.list_label')}>
        {isLoading ? (
          <div className="transcript-version-empty">
            <Loader2 size={16} className="queue-icon-spin" />
            {t('versions.loading')}
          </div>
        ) : snapshots.length === 0 ? (
          <div className="transcript-version-empty">{t('versions.empty')}</div>
        ) : (
          snapshots.map((snapshot) => (
            <button
              key={snapshot.id}
              type="button"
              className={`transcript-version-item ${snapshot.id === selectedSnapshotId ? 'active' : ''}`}
              onClick={() => {
                setSelectedSnapshotId(snapshot.id);
                setSelectedRowIds(new Set());
              }}
              disabled={isBusy}
            >
              <span className="transcript-version-item-title">
                {getReasonLabel(snapshot.reason, t)}
              </span>
              <span className="transcript-version-item-meta">
                <Clock3 size={13} />
                {new Date(snapshot.createdAt).toLocaleString()}
              </span>
              <span className="transcript-version-item-meta">
                {t('versions.segment_count', { count: snapshot.segmentCount })}
              </span>
            </button>
          ))
        )}
      </aside>

      <section className="transcript-version-diff" aria-label={t('versions.diff_label')}>
        {selectedRecord ? (
          isDiffLoading ? (
            <div className="transcript-version-empty">
              <Loader2 size={16} className="queue-icon-spin" />
              {t('versions.loading')}
            </div>
          ) : (
            <>
              <div className="transcript-version-diff-toolbar">
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={handleToggleAll}
                  disabled={isBusy || isDiffLoading || changedRows.length === 0}
                >
                  {allChangedRowsSelected ? <CheckSquare size={14} /> : <Square size={14} />}
                  {allChangedRowsSelected ? t('versions.clear_selection') : t('versions.select_all_changes')}
                </button>
                <span>
                  {t('versions.selected_count', { count: selectedRowIds.size })}
                </span>
              </div>

              <div className="transcript-version-diff-list">
                {diffRows.map((row) => {
                  const isChanged = row.status !== 'unchanged';
                  const checked = selectedRowIds.has(row.id);
                  return (
                    <article
                      key={row.id}
                      className={`transcript-version-diff-row is-${row.status}`}
                    >
                      <div className="transcript-version-diff-row-head">
                        <button
                          type="button"
                          className="btn btn-icon btn-sm transcript-version-check"
                          onClick={() => toggleRow(row.id)}
                          disabled={!isChanged || isBusy}
                          aria-label={getStatusLabel(row.status, t)}
                        >
                          {checked ? <CheckSquare size={15} /> : <Square size={15} />}
                        </button>
                        <span className="transcript-version-status">
                          {getStatusLabel(row.status, t)}
                        </span>
                        <span className="transcript-version-time">
                          {formatSegmentTime(row.currentSegment || row.snapshotSegment)}
                        </span>
                      </div>
                      <div className="transcript-version-diff-columns">
                        <div className="transcript-version-diff-cell">
                          <span>{t('versions.before')}</span>
                          <p>{getSegmentText(row.snapshotSegment) || t('versions.empty_segment')}</p>
                        </div>
                        <div className="transcript-version-diff-cell">
                          <span>{t('versions.after')}</span>
                          <p>{getSegmentText(row.currentSegment) || t('versions.empty_segment')}</p>
                        </div>
                      </div>
                    </article>
                  );
                })}
              </div>
            </>
          )
        ) : (
          <div className="transcript-version-empty">{t('versions.choose_snapshot')}</div>
        )}
      </section>
    </PanelModal>
  );
}
