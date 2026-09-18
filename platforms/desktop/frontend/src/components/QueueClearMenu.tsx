import type React from 'react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckIcon, ChevronDownIcon, TrashIcon } from './Icons';

export type QueueClearScope = 'completed' | 'all';

export interface QueueClearMenuProps {
  completedCount: number;
  totalCount: number;
  onClear: (scope: QueueClearScope) => void;
  disabled?: boolean;
  className?: string;
}

/**
 * Dropdown menu for queue clear actions (Clear Completed vs Clear All).
 */
export function QueueClearMenu({
  completedCount,
  totalCount,
  onClear,
  disabled = false,
  className = '',
}: QueueClearMenuProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!isOpen) return undefined;

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
    if (!isOpen) return undefined;

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

  const handleSelect = (scope: QueueClearScope) => {
    setIsOpen(false);
    onClear(scope);
  };

  const isTriggerDisabled = totalCount === 0 || disabled;
  const isCompletedDisabled = completedCount === 0 || disabled;

  return (
    <div className={`queue-clear-menu ${className}`} ref={menuRef}>
      <button
        ref={triggerRef}
        type="button"
        className={`btn btn-secondary-soft btn-sm queue-clear-trigger${isOpen ? ' is-open' : ''}`}
        onClick={() => setIsOpen((open) => !open)}
        disabled={isTriggerDisabled}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={menuId}
        aria-label={t('batch.clear_options', { defaultValue: 'Clear options' })}
        data-tooltip={
          isOpen ? undefined : t('batch.clear_options', { defaultValue: 'Clear options' })
        }
        data-tooltip-pos="left"
        data-testid="queue-clear-trigger"
      >
        <TrashIcon width={13} height={13} />
        <ChevronDownIcon
          className={`queue-clear-chevron${isOpen ? ' is-open' : ''}`}
          width={11}
          height={11}
          aria-hidden="true"
        />
      </button>

      {isOpen ? (
        <div
          id={menuId}
          className="queue-clear-dropdown"
          role="menu"
          aria-label={t('batch.clear_options', { defaultValue: 'Clear options' })}
        >
          {/* Clear Completed */}
          <button
            type="button"
            role="menuitem"
            className="queue-clear-dropdown-item"
            disabled={isCompletedDisabled}
            onClick={() => handleSelect('completed')}
          >
            <span
              className="queue-clear-item-icon queue-clear-item-icon-success"
              aria-hidden="true"
            >
              <CheckIcon width={14} height={14} />
            </span>
            <span className="queue-clear-item-content">
              <span className="queue-clear-item-title">
                {t('batch.clear_completed', { defaultValue: 'Clear Completed' })}
              </span>
              <span className="queue-clear-item-hint">
                {t('batch.clear_completed_hint', {
                  defaultValue: 'Clear all completed files ({{count}})',
                  count: completedCount,
                })}
              </span>
            </span>
          </button>

          <div className="queue-clear-dropdown-divider" role="separator" />

          {/* Clear All */}
          <button
            type="button"
            role="menuitem"
            className="queue-clear-dropdown-item queue-clear-dropdown-item-danger"
            disabled={isTriggerDisabled}
            onClick={() => handleSelect('all')}
          >
            <span className="queue-clear-item-icon queue-clear-item-icon-danger" aria-hidden="true">
              <TrashIcon width={14} height={14} />
            </span>
            <span className="queue-clear-item-content">
              <span className="queue-clear-item-title">
                {t('batch.clear_all', { defaultValue: 'Clear All' })}
              </span>
              <span className="queue-clear-item-hint">
                {t('batch.clear_all_hint', {
                  defaultValue: 'Clear all files in queue ({{count}})',
                  count: totalCount,
                })}
              </span>
            </span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
