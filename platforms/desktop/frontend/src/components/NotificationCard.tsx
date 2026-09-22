import type React from 'react';
import { useTranslation } from 'react-i18next';
import type { NotificationAction, NotificationEntry } from '../types/notification';
import { CloseIcon } from './Icons';

function getActionButtonClassName(action: NotificationAction): string {
  const variantClassName =
    action.variant === 'primary'
      ? 'btn-primary'
      : action.variant === 'secondary'
        ? 'btn-secondary'
        : 'btn-secondary-soft';

  return `btn ${variantClassName} btn-sm notification-center-item-action`;
}

function renderActions(actions: NotificationAction[]): React.ReactNode {
  if (actions.length === 0) {
    return null;
  }

  return actions.map((action) => (
    <button
      key={action.id}
      type="button"
      className={getActionButtonClassName(action)}
      onClick={() => {
        if (!action.disabled) {
          void action.run();
        }
      }}
      disabled={action.disabled}
    >
      {action.label}
    </button>
  ));
}

function renderCloseButton(action?: NotificationAction): React.ReactNode {
  if (!action) {
    return null;
  }

  return (
    <button
      type="button"
      className="btn btn-icon notification-center-item-close"
      onClick={() => {
        if (!action.disabled) {
          void action.run();
        }
      }}
      aria-label={action.label}
      disabled={action.disabled}
    >
      <CloseIcon />
    </button>
  );
}

export function NotificationCard({ entry }: { entry: NotificationEntry }): React.JSX.Element {
  const { t } = useTranslation();
  const mainClassName = entry.onOpen
    ? 'notification-center-item-main'
    : 'notification-center-item-main notification-center-item-main-static';

  const copy = (
    <>
      <span className="notification-center-item-icon" aria-hidden="true">
        {entry.icon}
      </span>
      <span className="notification-center-item-copy">
        <strong className="notification-center-item-title">{entry.title}</strong>
        {entry.body ? (
          <span
            className={`notification-center-item-body${entry.bodyClassName ? ` ${entry.bodyClassName}` : ''}`}
          >
            {entry.body}
          </span>
        ) : null}
      </span>
    </>
  );

  const hasActions = entry.actions.length > 0;

  return (
    <li
      className={`notification-center-item notification-center-item-tone-${entry.tone}${entry.itemClassName ? ` ${entry.itemClassName}` : ''}`}
    >
      <div className="notification-center-item-header">
        {entry.onOpen ? (
          <button type="button" className={mainClassName} onClick={entry.onOpen}>
            {copy}
          </button>
        ) : (
          <div className={mainClassName}>{copy}</div>
        )}
        {renderCloseButton(entry.closeAction)}
      </div>
      {entry.progress != null ? (
        <div className="notification-center-item-support">
          <div className="update-progress-container notification-center-update-progress">
            <div className="update-progress-header">
              <span>{t('task_center.progress', { defaultValue: 'Progress' })}</span>
              <span>{Math.round(entry.progress)}%</span>
            </div>
            <div className="progress-bar">
              <div
                className="progress-bar-fill"
                style={{ width: `${Math.max(0, Math.min(entry.progress, 100))}%` }}
              />
            </div>
          </div>
        </div>
      ) : null}
      {entry.support ? (
        <div className="notification-center-item-support">{entry.support}</div>
      ) : null}
      {entry.detail ? (
        <div className="notification-center-item-support">
          <div className="notification-center-item-detail">{entry.detail}</div>
        </div>
      ) : null}
      {hasActions ? (
        <div className="notification-center-item-actions">{renderActions(entry.actions)}</div>
      ) : null}
    </li>
  );
}
