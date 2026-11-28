import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NotificationAction, NotificationEntry } from '../types/notification';
import { CheckIcon, ChevronDownIcon, CloseIcon, CopyIcon } from './Icons';
import { Button, type ButtonTier, IconButton } from './ui/Button';

function getActionButtonTier(action: NotificationAction): ButtonTier {
  if (action.variant === 'primary') return 'primary';
  if (action.variant === 'secondary') return 'secondary';
  return 'ghost';
}

function renderActions(actions: NotificationAction[]): React.ReactNode {
  if (actions.length === 0) {
    return null;
  }

  return actions.map((action) => (
    <Button
      key={action.id}
      tier={getActionButtonTier(action)}
      size="sm"
      className="notification-center-item-action"
      onClick={() => {
        if (!action.disabled) {
          void action.run();
        }
      }}
      disabled={action.disabled}
    >
      {action.label}
    </Button>
  ));
}

function renderCloseButton(action?: NotificationAction): React.ReactNode {
  if (!action) {
    return null;
  }

  return (
    <IconButton
      tier="ghost"
      size="xs"
      className="notification-center-item-close"
      icon={<CloseIcon />}
      onClick={() => {
        if (!action.disabled) {
          void action.run();
        }
      }}
      aria-label={action.label}
      tooltipPos="left"
      disabled={action.disabled}
    />
  );
}
export function NotificationCard({
  entry,
  isFocused = false,
}: {
  entry: NotificationEntry;
  isFocused?: boolean;
}): React.JSX.Element {
  const { t } = useTranslation();
  const [isBodyExpanded, setIsBodyExpanded] = useState(entry.defaultExpanded ?? false);
  const [isDetailExpanded, setIsDetailExpanded] = useState(entry.defaultExpanded ?? false);
  const [isItemsExpanded, setIsItemsExpanded] = useState(entry.defaultExpanded ?? false);
  const [isCopied, setIsCopied] = useState(false);
  const cardRef = useRef<HTMLLIElement>(null);
  const copyTimeoutRef = useRef<number | null>(null);
  const prevFocusedIdRef = useRef<string | null>(null);
  const hasSubItems = Boolean(entry.items && entry.items.length > 0);

  useEffect(() => {
    return () => {
      clearTimeout(copyTimeoutRef.current ?? undefined);
    };
  }, []);

  useEffect(() => {
    const focusKey = isFocused ? entry.id : null;
    if (!focusKey || prevFocusedIdRef.current === focusKey) {
      if (!focusKey) {
        prevFocusedIdRef.current = null;
      }
      return;
    }
    prevFocusedIdRef.current = focusKey;
    if (hasSubItems) {
      setIsItemsExpanded(true);
    }
    cardRef.current?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' });
  }, [isFocused, entry.id, hasSubItems]);

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
        <Button
          tier="ghost"
          size="xs"
          className="notification-center-expand-btn"
          rightIcon={
            <ChevronDownIcon
              className={`notification-center-expand-btn-icon${isBodyExpanded ? ' is-expanded' : ''}`}
            />
          }
          onClick={toggleBodyExpand}
          onKeyDown={(e) => {
            e.stopPropagation();
          }}
          aria-expanded={isBodyExpanded}
        >
          <span>{isBodyExpanded ? t('task_center.collapse') : t('task_center.expand')}</span>
        </Button>
      ) : null}
    </div>
  ) : null;

  const copy = (
    <>
      <span className="notification-center-item-icon" aria-hidden="true">
        {entry.icon}
      </span>
      <span className="notification-center-item-copy">
        <span className="notification-center-item-title-row">
          <strong className="notification-center-item-title">{entry.title}</strong>
          {entry.timestampLabel ? (
            <span className="notification-center-item-time">{entry.timestampLabel}</span>
          ) : null}
        </span>
        {bodyElement}
      </span>
    </>
  );

  const hasActions = entry.actions.length > 0;

  return (
    <li
      ref={cardRef}
      className={`notification-center-item notification-center-item-tone-${entry.tone}${
        entry.itemClassName ? ` ${entry.itemClassName}` : ''
      }${isFocused ? ' is-focused' : ''}${
        isCollapsible ? ' notification-center-item-collapsible' : ''
      }${isCollapsible && isAnyExpanded ? ' is-expanded' : ''}${
        isCollapsible && !isAnyExpanded ? ' is-collapsed' : ''
      }`}
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
      {hasSubItems && entry.items ? (
        <div className="notification-center-sublist-wrapper">
          <Button
            tier="ghost"
            size="xs"
            className="notification-center-expand-btn notification-center-sublist-toggle"
            rightIcon={
              <ChevronDownIcon
                className={`notification-center-expand-btn-icon${isItemsExpanded ? ' is-expanded' : ''}`}
              />
            }
            onClick={(e) => {
              e.stopPropagation();
              setIsItemsExpanded((prev) => !prev);
            }}
            onKeyDown={(e) => {
              e.stopPropagation();
            }}
            aria-expanded={isItemsExpanded}
          >
            <span>
              {isItemsExpanded
                ? t('task_center.items_collapse', {
                    count: entry.items.length,
                    defaultValue: 'Hide files ({{count}})',
                  })
                : (entry.itemsLabel ??
                  t('task_center.items_expand', {
                    count: entry.items.length,
                    defaultValue: 'Show files ({{count}})',
                  }))}
            </span>
          </Button>
          {isItemsExpanded ? (
            <ul className="notification-center-sublist">
              {entry.items.map((subItem) => (
                <li
                  key={subItem.id}
                  className={`notification-center-subitem notification-center-item-tone-${subItem.tone}`}
                >
                  <span className="notification-center-subitem-dot" aria-hidden="true" />
                  <div className="notification-center-subitem-content">
                    <div className="notification-center-subitem-main">
                      <span className="notification-center-subitem-title">{subItem.title}</span>
                      {subItem.progress != null ? (
                        <span className="notification-center-subitem-progress">
                          {Math.round(subItem.progress)}%
                        </span>
                      ) : null}
                    </div>
                    {subItem.subtitle ? (
                      <div className="notification-center-subitem-subtitle">{subItem.subtitle}</div>
                    ) : null}
                    {subItem.detail ? (
                      <div className="notification-center-subitem-detail">{subItem.detail}</div>
                    ) : null}
                  </div>
                  {subItem.actions && subItem.actions.length > 0 ? (
                    <div className="notification-center-subitem-actions">
                      {subItem.actions.map((subAction) => (
                        <Button
                          key={subAction.id}
                          tier={getActionButtonTier(subAction)}
                          size="xs"
                          className="notification-center-subitem-action-btn"
                          disabled={subAction.disabled}
                          onClick={(e) => {
                            e.stopPropagation();
                            if (!subAction.disabled) {
                              void subAction.run();
                            }
                          }}
                        >
                          {subAction.label}
                        </Button>
                      ))}
                    </div>
                  ) : null}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
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
                <Button
                  tier="ghost"
                  size="xs"
                  className="notification-center-expand-btn"
                  rightIcon={
                    <ChevronDownIcon
                      className={`notification-center-expand-btn-icon${
                        isDetailExpanded ? ' is-expanded' : ''
                      }`}
                    />
                  }
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
                </Button>
              ) : null}
              {showDetailCopyButton ? (
                <Button
                  tier="ghost"
                  size="xs"
                  feedback={isCopied ? 'success' : 'idle'}
                  className="notification-center-detail-copy-btn"
                  leftIcon={
                    isCopied ? (
                      <CheckIcon className="notification-center-copy-icon" />
                    ) : (
                      <CopyIcon className="notification-center-copy-icon" />
                    )
                  }
                  onClick={handleCopyDetail}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                  }}
                  aria-label={t('task_center.copy_detail')}
                >
                  <span>{isCopied ? t('task_center.copied') : t('task_center.copy_detail')}</span>
                </Button>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}
