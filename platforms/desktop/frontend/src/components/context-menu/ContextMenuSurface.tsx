import React, { useCallback, useLayoutEffect, useRef } from 'react';
import { ModalPortal } from '../ModalPortal';
import type {
  ContextMenuAction,
  ContextMenuCloseReason,
} from './types';

interface ContextMenuSurfaceProps {
  actions: ContextMenuAction[];
  ariaLabel: string;
  onAction: (action: ContextMenuAction) => void;
  onDismiss: (reason: ContextMenuCloseReason) => void;
  point: { x: number; y: number };
}

export function ContextMenuSurface({
  actions,
  ariaLabel,
  onAction,
  onDismiss,
  point,
}: ContextMenuSurfaceProps): React.JSX.Element {
  const menuRef = useRef<HTMLDivElement>(null);

  const getEnabledItems = useCallback(() => Array.from(
    menuRef.current?.querySelectorAll<HTMLButtonElement>('.context-menu-item:not(:disabled)') ?? [],
  ), []);

  useLayoutEffect(() => {
    const firstEnabledItem = getEnabledItems()[0];
    if (firstEnabledItem) {
      firstEnabledItem.focus();
      return;
    }

    menuRef.current?.focus();
  }, [actions, getEnabledItems]);

  useLayoutEffect(() => {
    const element = menuRef.current;
    if (!element) {
      return;
    }

    const margin = 8;
    const rect = element.getBoundingClientRect();
    const maxLeft = Math.max(margin, window.innerWidth - rect.width - margin);
    const maxTop = Math.max(margin, window.innerHeight - rect.height - margin);
    const left = Math.max(margin, Math.min(point.x, maxLeft));
    const top = Math.max(margin, Math.min(point.y, maxTop));

    element.style.left = `${left}px`;
    element.style.top = `${top}px`;
  }, [point.x, point.y]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onDismiss('escape');
      return;
    }

    if (event.key === 'Tab') {
      onDismiss('outside');
      return;
    }

    const items = getEnabledItems();
    if (items.length === 0) {
      return;
    }

    const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        items[(currentIndex + 1 + items.length) % items.length]?.focus();
        break;
      case 'ArrowUp':
        event.preventDefault();
        items[(currentIndex - 1 + items.length) % items.length]?.focus();
        break;
      case 'Home':
        event.preventDefault();
        items[0]?.focus();
        break;
      case 'End':
        event.preventDefault();
        items[items.length - 1]?.focus();
        break;
      default:
        break;
    }
  };

  return (
    <ModalPortal>
      <div
        ref={menuRef}
        className="context-menu"
        data-context-menu-surface="true"
        role="menu"
        aria-label={ariaLabel}
        tabIndex={-1}
        onKeyDown={handleKeyDown}
        style={{ left: point.x, top: point.y }}
      >
        {actions.map((action) => (
          <button
            key={action.id}
            type="button"
            className={[
              'context-menu-item',
              action.tone === 'danger' ? 'context-menu-item--danger' : '',
              action.dividerBefore ? 'context-menu-item--with-divider' : '',
            ].filter(Boolean).join(' ')}
            role="menuitem"
            aria-label={action.label}
            disabled={action.disabled}
            onClick={() => onAction(action)}
            tabIndex={-1}
          >
            <span className="context-menu-item-icon" aria-hidden="true">
              {action.icon}
            </span>
            <span className="context-menu-item-label">{action.label}</span>
            {action.shortcut && (
              <span className="context-menu-item-shortcut" aria-hidden="true">
                {action.shortcut}
              </span>
            )}
          </button>
        ))}
      </div>
    </ModalPortal>
  );
}
