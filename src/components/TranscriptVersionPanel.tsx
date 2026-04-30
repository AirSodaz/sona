import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckSquare,
  Clock3,
  FileDiff,
  Loader2,
  RotateCcw,
  Square,
  Undo2,
  X,
} from 'lucide-react';
import { useDialogStore } from '../stores/dialogStore';
import { useHistoryStore } from '../stores/historyStore';
import { setTranscriptSegments } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { transcriptSnapshotService } from '../services/transcriptSnapshotService';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotRecord,
  TranscriptSnapshotReason,
} from '../types/transcriptSnapshot';
import {
  buildTranscriptDiffRows,
  countChangedTranscriptDiffRows,
  restoreSelectedTranscriptDiffRows,
} from '../utils/transcriptDiff';
import './PanelModal.css';
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
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const updateTranscript = useHistoryStore((state) => state.updateTranscript);
  const currentSegments = useTranscriptSessionStore((state) => state.segments);
  const [snapshots, setSnapshots] = useState<TranscriptSnapshotMetadata[]>([]);
  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string | null>(null);
  const [selectedRecord, setSelectedRecord] = useState<TranscriptSnapshotRecord | null>(null);
  const [selectedRowIds, setSelectedRowIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadSnapshots = useCallback(async (preferredSnapshotId?: string | null) => {
    if (!historyId) {
      setSnapshots([]);
      setSelectedSnapshotId(null);
      setSelectedRowIds(new Set());
      return;
    }

    setIsLoading(true);
    setError(null);
    try {
      const nextSnapshots = await transcriptSnapshotService.listSnapshots(historyId);
      setSnapshots(nextSnapshots);
      const nextSelectedId = preferredSnapshotId
        && nextSnapshots.some((snapshot) => snapshot.id === preferredSnapshotId)
        ? preferredSnapshotId
        : nextSnapshots[0]?.id || null;
      setSelectedSnapshotId(nextSelectedId);
      setSelectedRowIds(new Set());
    } catch {
      setError(t('versions.error_load'));
      setSnapshots([]);
      setSelectedSnapshotId(null);
      setSelectedRowIds(new Set());
    } finally {
      setIsLoading(false);
    }
  }, [historyId, t]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) {
        void loadSnapshots();
      }
    });

    return () => {
      cancelled = true;
    };
  }, [isOpen, loadSnapshots]);

  useEffect(() => {
    let cancelled = false;

    queueMicrotask(() => {
      if (cancelled) {
        return;
      }

      if (!isOpen || !historyId || !selectedSnapshotId) {
        setSelectedRecord(null);
        return;
      }

      setError(null);
      setSelectedRecord(null);

      void transcriptSnapshotService.loadSnapshot(historyId, selectedSnapshotId)
        .then((record) => {
          if (!cancelled) {
            setSelectedRecord(record);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setError(t('versions.error_load_snapshot'));
          }
        });
    });

    return () => {
      cancelled = true;
    };
  }, [historyId, isOpen, selectedSnapshotId, t]);

  const diffRows = useMemo(() => (
    selectedRecord ? buildTranscriptDiffRows(selectedRecord.segments, currentSegments) : []
  ), [currentSegments, selectedRecord]);

  const changedRows = useMemo(
    () => diffRows.filter((row) => row.status !== 'unchanged'),
    [diffRows],
  );
  const changedCount = countChangedTranscriptDiffRows(diffRows);

  const toggleRow = (rowId: string) => {
    setSelectedRowIds((current) => {
      const next = new Set(current);
      if (next.has(rowId)) {
        next.delete(rowId);
      } else {
        next.add(rowId);
      }
      return next;
    });
  };

  const handleToggleAll = () => {
    setSelectedRowIds((current) => (
      current.size === changedRows.length
        ? new Set()
        : new Set(changedRows.map((row) => row.id))
    ));
  };

  const persistRestoredSegments = async (nextSegments: TranscriptSegment[]) => {
    if (!historyId) {
      return;
    }

    await updateTranscript(historyId, nextSegments);
    setTranscriptSegments(nextSegments);
  };

  const handleRestoreSelected = async () => {
    if (!historyId || !selectedRecord || selectedRowIds.size === 0) {
      return;
    }

    const confirmed = await confirm(
      t('versions.restore_selected_confirm', { count: selectedRowIds.size }),
      {
        title: t('versions.restore_selected'),
        confirmLabel: t('versions.restore_selected'),
        variant: 'warning',
      },
    );
    if (!confirmed) {
      return;
    }

    setIsBusy(true);
    setError(null);
    try {
      const latestSegments = useTranscriptSessionStore.getState().segments;
      const latestRows = buildTranscriptDiffRows(selectedRecord.segments, latestSegments);
      await transcriptSnapshotService.createSnapshot(historyId, 'restore', latestSegments);
      await persistRestoredSegments(restoreSelectedTranscriptDiffRows(latestRows, selectedRowIds));
      await loadSnapshots();
    } catch (restoreError) {
      await showError({
        code: 'versions.restore_selected_failed',
        messageKey: 'versions.error_restore_selected',
        cause: restoreError,
      });
    } finally {
      setIsBusy(false);
    }
  };

  const handleRestoreAll = async () => {
    if (!historyId || !selectedRecord) {
      return;
    }

    const confirmed = await confirm(
      t('versions.restore_all_confirm'),
      {
        title: t('versions.restore_all'),
        confirmLabel: t('versions.restore_all'),
        variant: 'warning',
      },
    );
    if (!confirmed) {
      return;
    }

    setIsBusy(true);
    setError(null);
    try {
      const latestSegments = useTranscriptSessionStore.getState().segments;
      await transcriptSnapshotService.createSnapshot(historyId, 'restore', latestSegments);
      await persistRestoredSegments(selectedRecord.segments);
      await loadSnapshots();
    } catch (restoreError) {
      await showError({
        code: 'versions.restore_all_failed',
        messageKey: 'versions.error_restore_all',
        cause: restoreError,
      });
    } finally {
      setIsBusy(false);
    }
  };

  if (!isOpen) {
    return null;
  }

  const allChangedRowsSelected = changedRows.length > 0 && selectedRowIds.size === changedRows.length;

  return (
    <div className="settings-overlay panel-modal-overlay transcript-version-overlay" onClick={onClose}>
      <div
        className="panel-modal-shell transcript-version-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="transcript-version-title"
      >
        <div className="panel-modal-header transcript-version-header">
          <div className="panel-modal-header-copy">
            <div className="panel-modal-badge transcript-version-badge">
              <FileDiff size={16} />
              <span>{t('versions.badge')}</span>
            </div>
            <h2 id="transcript-version-title">{t('versions.title')}</h2>
            <p>{t('versions.description')}</p>
          </div>
          <div className="panel-modal-header-controls">
            <div className="panel-modal-toolbar transcript-version-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleRestoreSelected()}
                disabled={isBusy || selectedRowIds.size === 0}
              >
                {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <Undo2 size={14} />}
                {t('versions.restore_selected')}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleRestoreAll()}
                disabled={isBusy || !selectedRecord}
              >
                <RotateCcw size={14} />
                {t('versions.restore_all')}
              </button>
            </div>
            <button
              type="button"
              className="btn btn-icon panel-modal-close"
              onClick={onClose}
              aria-label={t('common.close', { defaultValue: 'Close' })}
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="panel-modal-meta-row transcript-version-meta-row">
          <span className="panel-modal-meta-label">{t('versions.snapshot_count')}</span>
          <span>{snapshots.length}</span>
          {selectedRecord ? (
            <>
              <span className="panel-modal-meta-label">{t('versions.changed_count')}</span>
              <span>{changedCount}</span>
            </>
          ) : null}
        </div>

        {error ? (
          <div className="transcript-version-error" role="alert">
            {error}
          </div>
        ) : null}

        <div className="panel-modal-content transcript-version-content">
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
              <>
                <div className="transcript-version-diff-toolbar">
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleToggleAll}
                    disabled={isBusy || changedRows.length === 0}
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
            ) : (
              <div className="transcript-version-empty">{t('versions.choose_snapshot')}</div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
