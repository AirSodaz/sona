import { FolderSync } from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAutomationStore } from '../../../../stores/automationStore';
import type { AutomationRule } from '../../../../types/automation';
import { SettingsTabContainer } from '../../../settings/SettingsLayout';
import { ToolHeroHeader } from '../../ToolHeroHeader';
import { AutomationRuleCard } from './AutomationRuleCard';
import { AutomationRuleModal } from './AutomationRuleModal';
import './ToolAutomationPanel.css';

export function ToolAutomationPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const rules = useAutomationStore((state) => state.rules);
  const runtimeStates = useAutomationStore((state) => state.runtimeStates);
  const processedEntries = useAutomationStore((state) => state.processedEntries);
  const deleteRule = useAutomationStore((state) => state.deleteRule);
  const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
  const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
  const retryFailed = useAutomationStore((state) => state.retryFailed);
  const retryFailedFile = useAutomationStore((state) => state.retryFailedFile);
  const [editing, setEditing] = useState<AutomationRule | null | undefined>(undefined);

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
    <SettingsTabContainer id="tools-panel-automation">
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
          <button type="button" className="btn btn-primary" onClick={() => setEditing(null)}>
            {t('automation.new_rule', { defaultValue: '+ New Rule' })}
          </button>
        }
      />
      {rules.length === 0 ? (
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
              onDelete={deleteRule}
              onToggle={toggleRuleEnabled}
              onScan={scanRuleNow}
              onRetryFailed={retryFailed}
              onRetryFile={retryFailedFile}
            />
          ))}
        </div>
      )}
      {editing !== undefined ? (
        <AutomationRuleModal rule={editing} onClose={() => setEditing(undefined)} />
      ) : null}
    </SettingsTabContainer>
  );
}
