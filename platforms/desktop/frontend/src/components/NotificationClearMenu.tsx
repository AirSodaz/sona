import type React from 'react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckIcon, ChevronDownIcon, TrashIcon } from './Icons';

export type ClearScope = 'succeeded' | 'all';

export interface NotificationClearMenuProps {
  succeededCount: number;
  totalClearableCount: number;
  onClear: (scope: ClearScope) => void;
  disabled?: boolean;
}

export function NotificationClearMenu({
  succeededCount,
  totalClearableCount,
  onClear,
  disabled = false,
}: NotificationClearMenuProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleClickOutside = (event: MouseEvent) => {
      if (
        menuRef.current &&
        !menuRef.current.contains(event.target as Node) &&
        !triggerRef.current?.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        setIsOpen(false);
        triggerRef.current?.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown, true);
    return () => {
      document.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [isOpen]);

  const handleSelect = (scope: ClearScope) => {
    setIsOpen(false);
    onClear(scope);
  };

  const isTriggerDisabled = totalClearableCount === 0 || disabled;
  const isSucceededDisabled = succeededCount === 0 || disabled;

  return (
    <div className="notification-clear-menu" ref={menuRef}>
      <button
        ref={triggerRef}
        type="button"
        className={`btn btn-secondary-soft btn-sm notification-clear-trigger${isOpen ? ' is-open' : ''}`}
        onClick={() => setIsOpen((open) => !open)}
        disabled={isTriggerDisabled}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={menuId}
        aria-label={t('task_center.clear', { defaultValue: 'Clear' })}
        data-testid="notification-clear-trigger"
      >
        <span>{t('task_center.clear', { defaultValue: 'Clear' })}</span>
        <ChevronDownIcon
          className={`notification-clear-chevron${isOpen ? ' is-open' : ''}`}
          aria-hidden="true"
        />
      </button>

      {isOpen ? (
        <div
          id={menuId}
          className="notification-clear-dropdown"
          role="menu"
          aria-label={t('task_center.clear_options', { defaultValue: 'Clear options' })}
        >
          <button
            type="button"
            role="menuitem"
            className="notification-clear-dropdown-item"
            disabled={isSucceededDisabled}
            onClick={() => handleSelect('succeeded')}
          >
            <span
              className="notification-clear-item-icon notification-clear-item-icon-success"
              aria-hidden="true"
            >
              <CheckIcon />
            </span>
            <span className="notification-clear-item-content">
              <span className="notification-clear-item-title">
                {t('task_center.clear_succeeded', { defaultValue: 'Clear succeeded' })}
              </span>
              <span className="notification-clear-item-hint">
                {t('task_center.clear_succeeded_hint', {
                  defaultValue: 'Only clear succeeded messages ({{count}})',
                  count: succeededCount,
                })}
              </span>
            </span>
          </button>

          <div className="notification-clear-dropdown-divider" role="separator" />

          <button
            type="button"
            role="menuitem"
            className="notification-clear-dropdown-item notification-clear-dropdown-item-danger"
            disabled={isTriggerDisabled}
            onClick={() => handleSelect('all')}
          >
            <span
              className="notification-clear-item-icon notification-clear-item-icon-danger"
              aria-hidden="true"
            >
              <TrashIcon />
            </span>
            <span className="notification-clear-item-content">
              <span className="notification-clear-item-title">
                {t('task_center.clear_all', { defaultValue: 'Clear all' })}
              </span>
              <span className="notification-clear-item-hint">
                {t('task_center.clear_all_hint', {
                  defaultValue: 'Clear all completed records ({{count}})',
                  count: totalClearableCount,
                })}
              </span>
            </span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
