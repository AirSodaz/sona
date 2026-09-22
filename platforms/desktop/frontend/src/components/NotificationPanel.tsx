import type React from 'react';
import { useTranslation } from 'react-i18next';
import type { NotificationGroup } from '../types/notification';
import { type ClearScope, NotificationClearMenu } from './NotificationClearMenu';
import { NotificationSection } from './NotificationSection';

interface NotificationPanelProps {
  panelId: string;
  grouped: NotificationGroup;
  hasEntries: boolean;
  succeededCount: number;
  totalClearableCount: number;
  onClear: (scope: ClearScope) => void;
}

export function NotificationPanel({
  panelId,
  grouped,
  hasEntries,
  succeededCount,
  totalClearableCount,
  onClear,
}: NotificationPanelProps): React.JSX.Element {
  const { t } = useTranslation();

  return (
    <div
      id={panelId}
      className="notification-center-panel"
      role="dialog"
      aria-label={t('task_center.panel_title', { defaultValue: 'Task Center' })}
    >
      <div className="notification-center-panel-header">
        <div className="notification-center-panel-title">
          {t('task_center.panel_title', { defaultValue: 'Task Center' })}
        </div>
        {totalClearableCount > 0 ? (
          <NotificationClearMenu
            succeededCount={succeededCount}
            totalClearableCount={totalClearableCount}
            onClear={onClear}
          />
        ) : null}
      </div>

      {!hasEntries ? (
        <div className="notification-center-empty">
          {t('task_center.empty', { defaultValue: 'No active tasks right now.' })}
        </div>
      ) : (
        <>
          <NotificationSection
            priority="action"
            title={t('task_center.needs_action', { defaultValue: 'Needs action' })}
            entries={grouped.action}
          />
          <NotificationSection
            priority="active"
            title={t('task_center.active', { defaultValue: 'Active' })}
            entries={grouped.active}
          />
          <NotificationSection
            priority="info"
            title={t('task_center.recent', { defaultValue: 'Recent' })}
            entries={grouped.info}
          />
        </>
      )}
    </div>
  );
}
