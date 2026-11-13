import { FolderSync } from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAutomationStore } from '../../../../stores/automationStore';
import { useDialogStore } from '../../../../stores/dialogStore';
import type { AutomationRule } from '../../../../types/automation';
import { SettingsTabContainer } from '../../../settings/SettingsLayout';
import { ToolHeroHeader } from '../../ToolHeroHeader';
import { AutomationRuleCard } from './AutomationRuleCard';
import { AutomationRuleModal } from './AutomationRuleModal';
import './ToolAutomationPanel.css';

export function ToolAutomationPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const rules = useAutomationStore((state) => state.rules);
  const isLoaded = useAutomationStore((state) => state.isLoaded);
  const error = useAutomationStore((state) => state.error);
  const loadAndStart = useAutomationStore((state) => state.loadAndStart);
  const runtimeStates = useAutomationStore((state) => state.runtimeStates);
  const processedEntries = useAutomationStore((state) => state.processedEntries);
  const deleteRule = useAutomationStore((state) => state.deleteRule);
  const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
  const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
  const retryFailed = useAutomationStore((state) => state.retryFailed);
  const retryFailedFile = useAutomationStore((state) => state.retryFailedFile);
  const showError = useDialogStore((state) => state.showError);
  const [editing, setEditing] = useState<AutomationRule | null | undefined>(undefined);

  const handleToggleRule = async (ruleId: string, enabled: boolean) => {
    try {
      await toggleRuleEnabled(ruleId, enabled);
    } catch (error) {
      await showError({
        code: 'automation.toggle_failed',
        messageKey: 'errors.automation.toggle_failed',
        cause: error,
      });
    }
  };

  const handleScanRule = async (ruleId: string) => {
    try {
      await scanRuleNow(ruleId);
    } catch (error) {
      await showError({
        code: 'automation.scan_failed',
        messageKey: 'errors.automation.scan_failed',
        cause: error,
      });
    }
  };

  const handleDeleteRule = async (ruleId: string) => {
    try {
      await deleteRule(ruleId);
    } catch (error) {
      await showError({
        code: 'automation.delete_failed',
        messageKey: 'errors.automation.delete_failed',
        cause: error,
      });
    }
  };

  const handleRetryFailed = async (ruleId: string) => {
    try {
      await retryFailed(ruleId);
    } catch (error) {
      await showError({
        code: 'automation.retry_failed',
        messageKey: 'errors.automation.retry_failed',
        cause: error,
      });
    }
  };

  const handleRetryFailedFile = async (ruleId: string, filePath: string) => {
    try {
      await retryFailedFile(ruleId, filePath);
    } catch (error) {
      await showError({
        code: 'automation.retry_failed',
        messageKey: 'errors.automation.retry_failed',
        cause: error,
      });
    }
  };

  const summary = useMemo(() => {
    const hasError = rules.some(
      (rule) =>
        runtimeStates[rule.id]?.status === 'error' ||
        processedEntries.some((entry) => entry.ruleId === rule.id && entry.status === 'error')
    );
    if (hasError) {
      return {
        tone: 'error' as const,
        text: t('automation.status.error', { defaultValue: 'Error' }),
      };
    }
    const activeCount = rules.filter((rule) => rule.enabled).length;
    if (activeCount > 0) {
      return {
        tone: 'active' as const,
        text: t('automation.watching_count', {
          defaultValue: 'Watching · {{count}}',
          count: activeCount,
        }),
      };
    }
    return {
      tone: 'inactive' as const,
      text: t('automation.status.stopped', { defaultValue: 'Stopped' }),
    };
  }, [processedEntries, rules, runtimeStates, t]);

  return (
    <SettingsTabContainer id="tools-panel-automation" ariaLabelledby="tools-tab-automation">
      <ToolHeroHeader
        icon={<FolderSync size={22} />}
        title={t('tools.automation', { defaultValue: 'Automation' })}
        description={t('tools.automation_desc', {
          defaultValue: 'Watch folders and send recordings into a project or export directory.',
        })}
        statusText={summary.text}
        statusTone={summary.tone}
        hideSwitch
        actions={
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setEditing(null)}
            disabled={!isLoaded}
          >
            {t('automation.new_rule', { defaultValue: '+ New Rule' })}
          </button>
        }
      />
      {!isLoaded && error ? (
        <div className="automation-empty">
          <p>{error}</p>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => {
              // The store records the failure and rethrows; the panel shows it from state.
              void loadAndStart().catch(() => undefined);
            }}
          >
            {t('common.retry', { defaultValue: 'Retry' })}
          </button>
        </div>
      ) : !isLoaded ? (
        <p className="automation-empty">{t('common.loading', { defaultValue: 'Loading...' })}</p>
      ) : rules.length === 0 ? (
        <p className="automation-empty">
          {t('automation.empty', { defaultValue: 'No automation rules yet.' })}
        </p>
      ) : (
        <div className="automation-rule-list">
          {rules.map((rule) => (
            <AutomationRuleCard
              key={rule.id}
              rule={rule}
              runtime={runtimeStates[rule.id]}
              processedEntries={processedEntries}
              onEdit={setEditing}
              onDelete={handleDeleteRule}
              onToggle={handleToggleRule}
              onScan={handleScanRule}
              onRetryFailed={handleRetryFailed}
              onRetryFile={handleRetryFailedFile}
            />
          ))}
        </div>
      )}
      {editing !== undefined ? (
        <AutomationRuleModal
          key={editing?.id ?? 'new'}
          rule={editing}
          onClose={() => setEditing(undefined)}
        />
      ) : null}
    </SettingsTabContainer>
  );
}
