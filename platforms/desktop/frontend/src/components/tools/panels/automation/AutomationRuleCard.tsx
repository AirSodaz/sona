import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useBatchQueueStore } from '../../../../stores/batchQueueStore';
import { useDialogStore } from '../../../../stores/dialogStore';
import { useProjectStore } from '../../../../stores/projectStore';
import type {
  AutomationProcessedEntry,
  AutomationRule,
  AutomationRuntimeState,
} from '../../../../types/automation';
import { Switch } from '../../../Switch';

interface AutomationRuleCardProps {
  rule: AutomationRule;
  runtime?: AutomationRuntimeState;
  processedEntries: AutomationProcessedEntry[];
  onEdit: (rule: AutomationRule) => void;
  onDelete: (ruleId: string) => Promise<void>;
  onToggle: (ruleId: string, enabled: boolean) => Promise<void>;
  onScan: (ruleId: string) => Promise<void>;
  onRetryFailed: (ruleId: string) => Promise<void>;
  onRetryFile: (ruleId: string, filePath: string) => Promise<void>;
}

function fileName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function AutomationRuleCard({
  rule,
  runtime,
  processedEntries,
  onEdit,
  onDelete,
  onToggle,
  onScan,
  onRetryFailed,
  onRetryFile,
}: AutomationRuleCardProps): React.JSX.Element {
  const { t } = useTranslation();
  const confirm = useDialogStore((state) => state.confirm);
  const projects = useProjectStore((state) => state.projects);
  const queueItems = useBatchQueueStore((state) => state.queueItems);
  const failedEntries = processedEntries.filter(
    (entry) => entry.ruleId === rule.id && entry.status === 'error'
  );
  const pendingCount = queueItems.filter(
    (item) => item.automationRuleId === rule.id && item.status === 'pending'
  ).length;
  const processingCount = queueItems.filter(
    (item) => item.automationRuleId === rule.id && item.status === 'processing'
  ).length;
  const blocked =
    runtime?.lastBlockedAt != null &&
    (runtime.lastQueuedAt == null || runtime.lastBlockedAt > runtime.lastQueuedAt);
  const status =
    runtime?.status === 'error' || failedEntries.length > 0
      ? 'error'
      : runtime?.status === 'scanning'
        ? 'scanning'
        : rule.enabled
          ? 'watching'
          : 'stopped';
  const projectName =
    !rule.projectId || rule.projectId === 'inbox'
      ? t('projects.inbox', { defaultValue: 'Inbox' })
      : projects.find((project) => project.id === rule.projectId)?.name || rule.projectId;
  const destination = rule.saveHistory
    ? projectName
    : `${rule.exportConfig.format.toUpperCase()} · ${rule.exportConfig.directory}`;
  const statusLabel =
    failedEntries.length > 0
      ? t('automation.failures', {
          defaultValue: '{{count}} failures',
          count: failedEntries.length,
        })
      : processingCount > 0
        ? t('automation.processing', {
            defaultValue: '{{count}} processing',
            count: processingCount,
          })
        : pendingCount > 0
          ? t('automation.pending', { defaultValue: '{{count}} pending', count: pendingCount })
          : t(`automation.status.${status}`, { defaultValue: status });

  return (
    <article className={`automation-rule-card is-${status}`}>
      <header className="automation-rule-card-header">
        <span className={`automation-status-dot is-${status}`} aria-hidden="true" />
        <div className="automation-rule-card-title">
          <strong>{rule.name}</strong>
          <span className="automation-status-chip">{statusLabel}</span>
        </div>
        <Switch
          checked={rule.enabled}
          onChange={(enabled) => void onToggle(rule.id, enabled)}
          aria-label={t('automation.toggle_rule', { defaultValue: 'Enable rule' })}
        />
      </header>
      <p className="automation-rule-flow">
        <span>{rule.watchDirectory}</span>
        {rule.recursive ? (
          <span className="automation-recursive-badge">
            {t('automation.recursive_badge', { defaultValue: 'subfolders' })}
          </span>
        ) : null}
        <span aria-hidden="true">→</span>
        <span>{destination}</span>
      </p>
      <div className="automation-rule-actions">
        <button type="button" className="btn btn-secondary" onClick={() => void onScan(rule.id)}>
          {t('automation.scan_now', { defaultValue: 'Scan Now' })}
        </button>
        <button type="button" className="btn btn-secondary" onClick={() => onEdit(rule)}>
          {t('common.edit', { defaultValue: 'Edit' })}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => {
            void confirm(
              t('automation.delete_confirm', {
                defaultValue: 'Delete automation rule "{{name}}"?',
                name: rule.name,
              }),
              {
                title: t('automation.delete_rule', { defaultValue: 'Delete Automation Rule' }),
                variant: 'error',
              }
            ).then((confirmed) => {
              if (confirmed) {
                return onDelete(rule.id);
              }
              return undefined;
            });
          }}
        >
          {t('common.delete', { defaultValue: 'Delete' })}
        </button>
      </div>
      {failedEntries.length > 0 || blocked ? (
        <div className="automation-rule-exceptions">
          {blocked && runtime?.lastBlockedReason ? (
            <p>
              {t(`automation.blocked.${runtime.lastBlockedReason}`, {
                defaultValue: runtime.lastBlockedReason,
              })}
            </p>
          ) : null}
          {failedEntries.map((entry) => (
            <div key={`${entry.filePath}-${entry.processedAt}`} className="automation-failed-entry">
              <div>
                <strong>{fileName(entry.filePath)}</strong>
                <span>{entry.errorMessage}</span>
              </div>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void onRetryFile(rule.id, entry.filePath)}
              >
                {t('automation.retry', { defaultValue: 'Retry' })}
              </button>
            </div>
          ))}
          {failedEntries.length > 0 ? (
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void onRetryFailed(rule.id)}
            >
              {t('automation.retry_all', { defaultValue: 'Retry All' })}
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}
