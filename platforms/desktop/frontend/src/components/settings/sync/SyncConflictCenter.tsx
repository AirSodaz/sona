import React from 'react';
import { AlertTriangle, Check, FileDiff, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { transcriptSnapshotService } from '../../../services/transcriptSnapshotService';
import {
  getSyncConflict,
  listSyncConflicts,
  resolveSyncConflict,
} from '../../../services/tauri/sync';
import { syncRuntimeService } from '../../../services/syncRuntimeService';
import { useDialogStore } from '../../../stores/dialogStore';
import type {
  SyncConflictDetail,
  SyncConflictResolution,
  SyncConflictSummary,
  SyncOperation,
} from '../../../types/sync';
import type { TranscriptSegment } from '../../../types/transcript';
import type { TranscriptDiffRow } from '../../../types/transcriptSnapshot';
import { SettingsAccordion } from '../SettingsLayout';

interface SyncConflictCenterProps {
  conflictCount: number;
  disabled: boolean;
}

function operationValue(operation: SyncOperation): unknown {
  return operation.kind.kind === 'set_field' ? operation.kind.value : undefined;
}

function transcriptDocument(operation: SyncOperation): TranscriptSegment[] | null {
  const value = operationValue(operation);
  return operation.entity.kind === 'history_transcript' && Array.isArray(value)
    ? value as TranscriptSegment[]
    : null;
}

function segmentText(segment: TranscriptSegment | undefined): string {
  if (!segment) {
    return '';
  }
  return segment.translation?.trim()
    ? `${segment.text}\n${segment.translation}`
    : segment.text;
}

function OperationValue({ operation }: { operation: SyncOperation }): React.JSX.Element {
  const { t } = useTranslation();
  if (operation.entity.kind === 'credential_profile') {
    return (
      <div className="sync-credential-conflict-value">
        <span>{operation.sourceDeviceId}</span>
        <time>{new Date(operation.version.clock.physical_ms).toLocaleString()}</time>
      </div>
    );
  }
  if (operation.kind.kind === 'delete_entity') {
    return <em>{t('settings.sync.conflict_deleted', { defaultValue: 'Deleted' })}</em>;
  }
  return <pre>{JSON.stringify(operation.kind.value, null, 2)}</pre>;
}

export function SyncConflictCenter({
  conflictCount,
  disabled,
}: SyncConflictCenterProps): React.JSX.Element {
  const { t } = useTranslation();
  const showError = useDialogStore((state) => state.showError);
  const [isOpen, setIsOpen] = React.useState(false);
  const [summaries, setSummaries] = React.useState<SyncConflictSummary[]>([]);
  const [detail, setDetail] = React.useState<SyncConflictDetail | null>(null);
  const [diffRows, setDiffRows] = React.useState<TranscriptDiffRow[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [resolving, setResolving] = React.useState(false);

  const reportError = React.useCallback((cause: unknown) => showError({
    code: 'sync.conflict_failed',
    messageKey: 'errors.sync.operation_failed',
    cause,
    titleKey: 'settings.sync.error_title',
  }), [showError]);

  const loadSummaries = React.useCallback(async () => {
    setLoading(true);
    try {
      const next = await listSyncConflicts();
      setSummaries(next);
    } catch (error) {
      await reportError(error);
    } finally {
      setLoading(false);
    }
  }, [reportError]);

  React.useEffect(() => {
    const timer = isOpen
      ? setTimeout(() => void loadSummaries(), 0)
      : null;
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
    };
  }, [isOpen, conflictCount, loadSummaries]);

  React.useEffect(() => {
    if (!detail) {
      return;
    }
    const current = transcriptDocument(detail.current);
    const conflicting = transcriptDocument(detail.conflicting);
    if (!current || !conflicting) {
      return;
    }
    let cancelled = false;
    void transcriptSnapshotService.buildDiff(conflicting, current)
      .then((result) => {
        if (!cancelled) {
          setDiffRows(result.rows);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          void reportError(error);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [detail, reportError]);

  const openDetail = async (summary: SyncConflictSummary) => {
    setLoading(true);
    setDiffRows([]);
    try {
      setDetail(await getSyncConflict(summary.conflictId));
    } catch (error) {
      await reportError(error);
    } finally {
      setLoading(false);
    }
  };

  const resolve = async (resolution: SyncConflictResolution) => {
    if (!detail) {
      return;
    }
    setResolving(true);
    try {
      await resolveSyncConflict(detail.summary.conflictId, resolution);
      setDetail(null);
      setDiffRows([]);
      await Promise.all([loadSummaries(), syncRuntimeService.refreshStatus()]);
    } catch (error) {
      await reportError(error);
    } finally {
      setResolving(false);
    }
  };

  const title = (
    <div className="settings-accordion-copy">
      <div className="settings-accordion-copy-title sync-conflict-title">
        <span>{t('settings.sync.conflict_center', { defaultValue: 'Conflict center' })}</span>
        {conflictCount > 0 ? <span className="sync-count-badge">{conflictCount}</span> : null}
      </div>
      <div className="settings-accordion-copy-hint">
        {t('settings.sync.conflict_center_hint', { defaultValue: 'Review concurrent edits that need a decision.' })}
      </div>
    </div>
  );

  return (
    <SettingsAccordion title={title} isOpen={isOpen} onToggle={() => !disabled && setIsOpen((value) => !value)}>
      <div className="sync-conflict-center">
        {loading ? (
          <div className="sync-empty-state"><Loader2 size={17} className="queue-icon-spin" />{t('common.loading', { defaultValue: 'Loading...' })}</div>
        ) : summaries.length === 0 ? (
          <div className="sync-empty-state"><Check size={17} />{t('settings.sync.no_conflicts', { defaultValue: 'No unresolved conflicts' })}</div>
        ) : (
          <div className="sync-conflict-layout">
            <div className="sync-conflict-list" aria-label={t('settings.sync.conflict_list', { defaultValue: 'Conflicts' })}>
              {summaries.map((summary) => (
                <button
                  type="button"
                  key={summary.conflictId}
                  className={detail?.summary.conflictId === summary.conflictId ? 'active' : ''}
                  onClick={() => void openDetail(summary)}
                >
                  <AlertTriangle size={15} />
                  <span>
                    <strong>{t(`settings.sync.entity_${summary.entity.kind}`, { defaultValue: summary.entity.kind })}</strong>
                    <small>{summary.field ?? t('settings.sync.entity_delete', { defaultValue: 'Delete' })} · {new Date(summary.createdAtMs).toLocaleString()}</small>
                  </span>
                </button>
              ))}
            </div>

            <div className="sync-conflict-detail">
              {detail ? (
                <>
                  <div className="sync-conflict-detail-heading"><FileDiff size={17} /><strong>{detail.summary.entity.id}</strong></div>
                  {diffRows.length > 0 ? (
                    <div className="sync-transcript-diff">
                      {diffRows.filter((row) => row.status !== 'unchanged').map((row) => (
                        <article key={row.id} className={`is-${row.status}`}>
                          <span>{t(`versions.diff.${row.status}`, { defaultValue: row.status })}</span>
                          <div><p>{segmentText(row.snapshotSegment)}</p><p>{segmentText(row.currentSegment)}</p></div>
                        </article>
                      ))}
                    </div>
                  ) : (
                    <div className="sync-conflict-values">
                      <div><span>{t('settings.sync.current_value', { defaultValue: 'Current' })}</span><OperationValue operation={detail.current} /></div>
                      <div><span>{t('settings.sync.conflicting_value', { defaultValue: 'Conflicting' })}</span><OperationValue operation={detail.conflicting} /></div>
                    </div>
                  )}
                  <div className="sync-actions">
                    <button type="button" className="btn btn-primary" disabled={resolving} onClick={() => void resolve('keep_current')}>{t('settings.sync.keep_current', { defaultValue: 'Keep current' })}</button>
                    <button type="button" className="btn btn-secondary" disabled={resolving} onClick={() => void resolve('use_conflicting')}>{t('settings.sync.use_conflicting', { defaultValue: 'Use conflicting' })}</button>
                    {detail.summary.entity.kind === 'history_transcript' ? (
                      <button type="button" className="btn btn-secondary" disabled={resolving} onClick={() => void resolve('keep_both')}>{t('settings.sync.keep_both', { defaultValue: 'Keep both' })}</button>
                    ) : null}
                  </div>
                </>
              ) : (
                <div className="sync-empty-state">{t('settings.sync.choose_conflict', { defaultValue: 'Select a conflict to compare versions' })}</div>
              )}
            </div>
          </div>
        )}
      </div>
    </SettingsAccordion>
  );
}
