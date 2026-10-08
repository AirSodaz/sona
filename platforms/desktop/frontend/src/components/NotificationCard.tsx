import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NotificationAction, NotificationEntry } from '../types/notification';
import { CheckIcon, ChevronDownIcon, CloseIcon, CopyIcon } from './Icons';

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
      data-tooltip={action.label}
      data-tooltip-pos="left"
      disabled={action.disabled}
    >
      <CloseIcon />
    </button>
  );
}
export function NotificationCard({ entry }: { entry: NotificationEntry }): React.JSX.Element {
  const { t } = useTranslation();
  const [isBodyExpanded, setIsBodyExpanded] = useState(entry.defaultExpanded ?? false);
  const [isDetailExpanded, setIsDetailExpanded] = useState(entry.defaultExpanded ?? false);
  const [isCopied, setIsCopied] = useState(false);
  const copyTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      clearTimeout(copyTimeoutRef.current ?? undefined);
    };
  }, []);

  const isBodyCollapsible = Boolean(
    entry.body &&
      (entry.body.split('\n').length > 3 ||
        entry.body.length > 140 ||
        entry.bodyClassName?.includes('update-body'))
  );

  const isDetailCollapsible = Boolean(
    entry.detail && (entry.detail.includes('\n') || entry.detail.length > 90)
  );

  const isCollapsible = isBodyCollapsible || isDetailCollapsible;
  const isAnyExpanded =
    (isBodyCollapsible && isBodyExpanded) || (isDetailCollapsible && isDetailExpanded);
  const showDetailCopyButton = Boolean(
    entry.detail && (isDetailCollapsible || entry.tone === 'error')
  );

  const toggleBodyExpand = (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsBodyExpanded((prev) => !prev);
  };

  const toggleDetailExpand = (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsDetailExpanded((prev) => !prev);
  };

  const handleCopyDetail = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!entry.detail) return;
    try {
      await navigator.clipboard.writeText(entry.detail);
      setIsCopied(true);
      clearTimeout(copyTimeoutRef.current ?? undefined);
      copyTimeoutRef.current = window.setTimeout(() => setIsCopied(false), 2000);
    } catch {
      // Ignore clipboard write failures
    }
  };

  const mainClassName = entry.onOpen
    ? 'notification-center-item-main'
    : 'notification-center-item-main notification-center-item-main-static';

  const bodyElement = entry.body ? (
    <div className="notification-center-item-body-container">
      <span
        className={`notification-center-item-body${entry.bodyClassName ? ` ${entry.bodyClassName}` : ''}${
          isBodyCollapsible ? ' is-collapsible' : ''
        }${isBodyCollapsible && !isBodyExpanded ? ' is-collapsed' : ''}${
          isBodyCollapsible && isBodyExpanded ? ' is-expanded' : ''
        }`}
      >
        {entry.body}
      </span>
      {isBodyCollapsible ? (
        <button
          type="button"
          className="notification-center-expand-btn"
          onClick={toggleBodyExpand}
          onKeyDown={(e) => {
            e.stopPropagation();
          }}
          aria-expanded={isBodyExpanded}
        >
          <span>{isBodyExpanded ? t('task_center.collapse') : t('task_center.expand')}</span>
          <ChevronDownIcon
            className={`notification-center-expand-btn-icon${isBodyExpanded ? ' is-expanded' : ''}`}
          />
        </button>
      ) : null}
    </div>
  ) : null;

  const copy = (
    <>
      <span className="notification-center-item-icon" aria-hidden="true">
        {entry.icon}
      </span>
      <span className="notification-center-item-copy">
        <strong className="notification-center-item-title">{entry.title}</strong>
        {bodyElement}
      </span>
    </>
  );

  const hasActions = entry.actions.length > 0;

  return (
    <li
      className={`notification-center-item notification-center-item-tone-${entry.tone}${
        entry.itemClassName ? ` ${entry.itemClassName}` : ''
      }${isCollapsible ? ' notification-center-item-collapsible' : ''}${
        isCollapsible && isAnyExpanded ? ' is-expanded' : ''
      }${isCollapsible && !isAnyExpanded ? ' is-collapsed' : ''}`}
    >
      <div className="notification-center-item-header">
        {entry.onOpen ? (
          <div
            role="button"
            tabIndex={0}
            className={mainClassName}
            onClick={entry.onOpen}
            onKeyDown={(e) => {
              if (e.target !== e.currentTarget) {
                return;
              }
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                entry.onOpen?.();
              }
            }}
          >
            {copy}
          </div>
        ) : (
          <div className={mainClassName}>{copy}</div>
        )}
        {entry.closeAction ? (
          <div className="notification-center-item-header-actions">
            {renderCloseButton(entry.closeAction)}
          </div>
        ) : null}
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
        <div
          className={`notification-center-item-detail-box${
            isDetailCollapsible ? ' is-collapsible' : ''
          }${isDetailCollapsible && isDetailExpanded ? ' is-expanded' : ''}${
            isDetailCollapsible && !isDetailExpanded ? ' is-collapsed' : ''
          }`}
        >
          <div
            className={`notification-center-item-detail${
              isDetailCollapsible && !isDetailExpanded ? ' is-collapsed' : ''
            }`}
          >
            {entry.detail}
          </div>
        </div>
      ) : null}
      {hasActions || isDetailCollapsible || showDetailCopyButton ? (
        <div className="notification-center-item-footer">
          {hasActions ? (
            <div className="notification-center-item-actions">{renderActions(entry.actions)}</div>
          ) : null}
          {isDetailCollapsible || showDetailCopyButton ? (
            <div className="notification-center-detail-actions">
              {isDetailCollapsible ? (
                <button
                  type="button"
                  className="notification-center-expand-btn"
                  onClick={toggleDetailExpand}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                  }}
                  aria-expanded={isDetailExpanded}
                >
                  <span>
                    {isDetailExpanded
                      ? t('task_center.collapse_detail')
                      : t('task_center.expand_detail')}
                  </span>
                  <ChevronDownIcon
                    className={`notification-center-expand-btn-icon${
                      isDetailExpanded ? ' is-expanded' : ''
                    }`}
                  />
                </button>
              ) : null}
              {showDetailCopyButton ? (
                <button
                  type="button"
                  className="notification-center-detail-copy-btn"
                  onClick={handleCopyDetail}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                  }}
                  aria-label={t('task_center.copy_detail')}
                >
                  {isCopied ? (
                    <CheckIcon className="notification-center-copy-icon" />
                  ) : (
                    <CopyIcon className="notification-center-copy-icon" />
                  )}
                  <span>{isCopied ? t('task_center.copied') : t('task_center.copy_detail')}</span>
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}
