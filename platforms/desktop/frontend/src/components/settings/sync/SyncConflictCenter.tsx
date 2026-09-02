import React from 'react';
import {
  AlertTriangle,
  BookOpen,
  Bot,
  Check,
  CheckCircle2,
  Copy,
  FileDiff,
  FileText,
  HardDrive,
  Laptop,
  Loader2,
  Settings as SettingsIcon,
  Tag,
  Trash2,
  User,
} from 'lucide-react';
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
  SyncEntityKind,
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
    ? (value as TranscriptSegment[])
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

function EntityKindIcon({ kind }: { kind: SyncEntityKind }): React.JSX.Element {
  switch (kind) {
    case 'history_item':
    case 'history_transcript':
    case 'transcript_snapshot':
      return <FileText size={15} />;
    case 'history_summary':
      return <BookOpen size={15} />;
    case 'setting':
      return <SettingsIcon size={15} />;
    case 'speaker_profile':
      return <User size={15} />;
    case 'vocabulary_set':
    case 'vocabulary_rule':
      return <BookOpen size={15} />;
    case 'tag':
    case 'project':
      return <Tag size={15} />;
    case 'automation_profile':
    case 'automation_rule':
      return <Bot size={15} />;
    default:
      return <AlertTriangle size={15} />;
  }
}

function FormattedConflictValue({ operation }: { operation: SyncOperation }): React.JSX.Element {
  const { t } = useTranslation();

  if (operation.kind.kind === 'delete_entity') {
    return (
      <div className="sync-conflict-deleted-badge">
        <Trash2 size={14} />
        <span>{t('settings.sync.conflict_deleted', { defaultValue: 'Entity deleted' })}</span>
      </div>
    );
  }

  const value = operation.kind.value;

  if (operation.entity.kind === 'credential_profile') {
    return (
      <div className="sync-credential-conflict-value">
        <Laptop size={14} />
        <span>{operation.sourceDeviceId}</span>
        <time>{new Date(operation.version.clock.physical_ms).toLocaleString()}</time>
      </div>
    );
  }

  if (typeof value === 'string') {
    return (
      <div className="sync-conflict-text-block">
        <p>{value}</p>
      </div>
    );
  }

  if (typeof value === 'boolean' || typeof value === 'number') {
    return (
      <div className="sync-conflict-primitive-block">
        <strong>{String(value)}</strong>
      </div>
    );
  }

  if (value && typeof value === 'object' && !Array.isArray(value)) {
    const entries = Object.entries(value as Record<string, unknown>);
    return (
      <div className="sync-conflict-kv-table">
        {entries.map(([k, v]) => (
          <div key={k} className="sync-conflict-kv-row">
            <span className="sync-conflict-key">{k}:</span>
            <span className="sync-conflict-val">
              {typeof v === 'object' ? JSON.stringify(v) : String(v)}
            </span>
          </div>
        ))}
      </div>
    );
  }

  if (Array.isArray(value)) {
    return (
      <div className="sync-conflict-array-list">
        <span className="sync-conflict-count">
          {t('settings.sync.items_count', { defaultValue: '{{count}} items', count: value.length })}
        </span>
        <pre className="sync-conflict-code">{JSON.stringify(value.slice(0, 5), null, 2)}</pre>
      </div>
    );
  }

  return <pre className="sync-conflict-code">{JSON.stringify(value, null, 2)}</pre>;
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

  const reportError = React.useCallback(
    (cause: unknown) =>
      showError({
        code: 'sync.conflict_failed',
        messageKey: 'errors.sync.operation_failed',
        cause,
        titleKey: 'settings.sync.error_title',
      }),
    [showError],
  );

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
    const timer = isOpen ? setTimeout(() => void loadSummaries(), 0) : null;
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
    void transcriptSnapshotService
      .buildDiff(conflicting, current)
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
        <span>{t('settings.sync.conflict_center', { defaultValue: 'Conflict Center' })}</span>
        {conflictCount > 0 && <span className="sync-count-badge">{conflictCount}</span>}
      </div>
      <div className="settings-accordion-copy-hint">
        {t('settings.sync.conflict_center_hint', {
          defaultValue: 'Review concurrent edits that need your decision.',
        })}
      </div>
    </div>
  );

  return (
    <SettingsAccordion
      title={title}
      isOpen={isOpen}
      onToggle={() => !disabled && setIsOpen((value) => !value)}
    >
      <div className="sync-conflict-center">
        {loading ? (
          <div className="sync-empty-state">
            <Loader2 size={17} className="queue-icon-spin" />
            {t('common.loading', { defaultValue: 'Loading...' })}
          </div>
        ) : summaries.length === 0 ? (
          <div className="sync-empty-state is-clean">
            <CheckCircle2 size={18} />
            {t('settings.sync.no_conflicts', { defaultValue: 'No unresolved conflicts. All data is in sync.' })}
          </div>
        ) : (
          <div className="sync-conflict-layout">
            {/* Conflict list */}
            <div
              className="sync-conflict-list"
              aria-label={t('settings.sync.conflict_list', { defaultValue: 'Conflicts' })}
            >
              {summaries.map((summary) => (
                <button
                  type="button"
                  key={summary.conflictId}
                  className={`sync-conflict-item-btn ${detail?.summary.conflictId === summary.conflictId ? 'active' : ''}`}
                  onClick={() => void openDetail(summary)}
                >
                  <EntityKindIcon kind={summary.entity.kind} />
                  <div className="sync-conflict-item-text">
                    <strong>
                      {t(`settings.sync.entity_${summary.entity.kind}`, {
                        defaultValue: summary.entity.kind,
                      })}
                    </strong>
                    <small>
                      {summary.field ?? t('settings.sync.entity_delete', { defaultValue: 'Delete' })} ·{' '}
                      {new Date(summary.createdAtMs).toLocaleTimeString()}
                    </small>
                  </div>
                </button>
              ))}
            </div>

            {/* Conflict Detail & Comparison */}
            <div className="sync-conflict-detail">
              {detail ? (
                <>
                  <div className="sync-conflict-detail-heading">
                    <FileDiff size={17} />
                    <strong>{detail.summary.entity.id}</strong>
                    <span className="sync-conflict-field-pill">
                      {detail.summary.field || t('settings.sync.full_entity', { defaultValue: 'Entity' })}
                    </span>
                  </div>

                  {diffRows.length > 0 ? (
                    <div className="sync-transcript-diff">
                      {diffRows
                        .filter((row) => row.status !== 'unchanged')
                        .map((row) => (
                          <article key={row.id} className={`is-${row.status}`}>
                            <span className="sync-diff-tag">
                              {t(`versions.diff.${row.status}`, { defaultValue: row.status })}
                            </span>
                            <div className="sync-diff-text-compare">
                              <p className="sync-diff-old">{segmentText(row.snapshotSegment)}</p>
                              <p className="sync-diff-new">{segmentText(row.currentSegment)}</p>
                            </div>
                          </article>
                        ))}
                    </div>
                  ) : (
                    <div className="sync-conflict-columns-grid">
                      {/* Left: Local / Current version */}
                      <div className="sync-conflict-version-col is-local">
                        <div className="sync-conflict-col-header">
                          <Laptop size={14} />
                          <span>{t('settings.sync.current_value', { defaultValue: 'Current device version' })}</span>
                        </div>
                        <div className="sync-conflict-col-body">
                          <FormattedConflictValue operation={detail.current} />
                        </div>
                      </div>

                      {/* Right: Remote / Conflicting version */}
                      <div className="sync-conflict-version-col is-remote">
                        <div className="sync-conflict-col-header">
                          <HardDrive size={14} />
                          <span>{t('settings.sync.conflicting_value', { defaultValue: 'Remote conflict version' })}</span>
                        </div>
                        <div className="sync-conflict-col-body">
                          <FormattedConflictValue operation={detail.conflicting} />
                        </div>
                      </div>
                    </div>
                  )}

                  {/* Resolution Action Bar */}
                  <div className="sync-conflict-actions-bar">
                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={resolving}
                      onClick={() => void resolve('keep_current')}
                    >
                      <Check size={16} />
                      <span>{t('settings.sync.keep_current', { defaultValue: 'Keep current version' })}</span>
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={resolving}
                      onClick={() => void resolve('use_conflicting')}
                    >
                      <span>{t('settings.sync.use_conflicting', { defaultValue: 'Use remote version' })}</span>
                    </button>
                    {detail.summary.entity.kind === 'history_transcript' && (
                      <button
                        type="button"
                        className="btn btn-secondary"
                        disabled={resolving}
                        onClick={() => void resolve('keep_both')}
                      >
                        <Copy size={16} />
                        <span>{t('settings.sync.keep_both', { defaultValue: 'Keep both (create duplicate)' })}</span>
                      </button>
                    )}
                  </div>
                </>
              ) : (
                <div className="sync-empty-state">
                  <FileDiff size={20} />
                  <span>{t('settings.sync.choose_conflict', { defaultValue: 'Select a conflict to compare versions' })}</span>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </SettingsAccordion>
  );
}

export default SyncConflictCenter;
