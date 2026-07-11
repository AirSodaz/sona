import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
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
} from '../types/transcriptSnapshot';

interface UseVersionPanelProps {
  isOpen: boolean;
  historyId: string | null;
}

export function useVersionPanel({ isOpen, historyId }: UseVersionPanelProps) {
  const { t } = useTranslation();
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const updateTranscript = useHistoryStore((state) => state.updateTranscript);
  const currentSegments = useTranscriptSessionStore((state) => state.segments);

  const [snapshots, setSnapshots] = useState<TranscriptSnapshotMetadata[]>([]);
  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string | null>(null);
  const [selectedRecord, setSelectedRecord] = useState<TranscriptSnapshotRecord | null>(null);
  const [diffRows, setDiffRows] = useState<TranscriptDiffRow[]>([]);
  const [changedCount, setChangedCount] = useState(0);
  const [selectedRowIds, setSelectedRowIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [isDiffLoading, setIsDiffLoading] = useState(false);
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

  useEffect(() => {
    let cancelled = false;

    queueMicrotask(() => {
      if (cancelled) {
        return;
      }

      if (!isOpen || !selectedRecord) {
        setDiffRows([]);
        setChangedCount(0);
        setIsDiffLoading(false);
        return;
      }

      setIsDiffLoading(true);
      setError(null);
      void transcriptSnapshotService.buildDiff(selectedRecord.segments, currentSegments)
        .then((result) => {
          if (cancelled) {
            return;
          }

          setDiffRows(result.rows);
          setChangedCount(result.changedCount);
          setSelectedRowIds((current) => {
            const changedRowIds = new Set(
              result.rows
                .filter((row) => row.status !== 'unchanged')
                .map((row) => row.id),
            );
            const next = new Set(
              Array.from(current).filter((rowId) => changedRowIds.has(rowId)),
            );
            return next.size === current.size ? current : next;
          });
        })
        .catch(() => {
          if (cancelled) {
            return;
          }

          setDiffRows([]);
          setChangedCount(0);
          setError(t('versions.error_load_snapshot'));
        })
        .finally(() => {
          if (!cancelled) {
            setIsDiffLoading(false);
          }
        });
    });

    return () => {
      cancelled = true;
    };
  }, [currentSegments, isOpen, selectedRecord, t]);

  const changedRows = useMemo(
    () => diffRows.filter((row) => row.status !== 'unchanged'),
    [diffRows],
  );

  const toggleRow = useCallback((rowId: string) => {
    setSelectedRowIds((current) => {
      const next = new Set(current);
      if (next.has(rowId)) {
        next.delete(rowId);
      } else {
        next.add(rowId);
      }
      return next;
    });
  }, []);

  const handleToggleAll = useCallback(() => {
    setSelectedRowIds((current) => (
      current.size === changedRows.length
        ? new Set()
        : new Set(changedRows.map((row) => row.id))
    ));
  }, [changedRows]);

  const persistRestoredSegments = useCallback(async (nextSegments: TranscriptSegment[]) => {
    if (!historyId) {
      return;
    }

    await updateTranscript(historyId, nextSegments);
    setTranscriptSegments(nextSegments);
  }, [historyId, updateTranscript]);

  const handleRestoreSelected = useCallback(async (): Promise<boolean> => {
    if (!historyId || !selectedRecord || selectedRowIds.size === 0) {
      return false;
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
      return false;
    }

    setIsBusy(true);
    setError(null);
    try {
      const latestSegments = useTranscriptSessionStore.getState().segments;
      const latestDiff = await transcriptSnapshotService.buildDiff(selectedRecord.segments, latestSegments);
      await transcriptSnapshotService.createSnapshot(historyId, 'restore', latestSegments);
      const restoredSegments = await transcriptSnapshotService.restoreDiffRows(latestDiff.rows, selectedRowIds);
      await persistRestoredSegments(restoredSegments);
      await loadSnapshots();
      return true;
    } catch (restoreError) {
      await showError({
        code: 'versions.restore_selected_failed',
        messageKey: 'versions.error_restore_selected',
        cause: restoreError,
      });
      return false;
    } finally {
      setIsBusy(false);
    }
  }, [confirm, historyId, loadSnapshots, persistRestoredSegments, selectedRecord, selectedRowIds, showError, t]);

  const handleRestoreAll = useCallback(async (): Promise<boolean> => {
    if (!historyId || !selectedRecord) {
      return false;
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
      return false;
    }

    setIsBusy(true);
    setError(null);
    try {
      const latestSegments = useTranscriptSessionStore.getState().segments;
      await transcriptSnapshotService.createSnapshot(historyId, 'restore', latestSegments);
      await persistRestoredSegments(selectedRecord.segments);
      await loadSnapshots();
      return true;
    } catch (restoreError) {
      await showError({
        code: 'versions.restore_all_failed',
        messageKey: 'versions.error_restore_all',
        cause: restoreError,
      });
      return false;
    } finally {
      setIsBusy(false);
    }
  }, [confirm, historyId, loadSnapshots, persistRestoredSegments, selectedRecord, showError, t]);

  return {
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
  };
}
