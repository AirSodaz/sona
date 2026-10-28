import type React from 'react';
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import '../../styles/split-button.css';
import { ChevronDownIcon } from '../Icons';
import { ModalPortal } from '../ModalPortal';

export interface SplitButtonItem {
  key: string;
  label: React.ReactNode;
  description?: React.ReactNode;
  icon?: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}

export interface SplitButtonProps {
  primaryLabel: React.ReactNode;
  primaryIcon?: React.ReactNode;
  onPrimaryClick: () => void;
  menuItems: SplitButtonItem[];
  disabled?: boolean;
  className?: string;
  size?: 'sm' | 'md';
  variant?: 'primary' | 'secondary';
  ariaLabel?: string;
  toggleAriaLabel?: string;
}

const useIsomorphicLayoutEffect = typeof window !== 'undefined' ? useLayoutEffect : useEffect;

export function SplitButton({
  primaryLabel,
  primaryIcon,
  onPrimaryClick,
  menuItems,
  disabled = false,
  className = '',
  size = 'sm',
  variant = 'primary',
  ariaLabel,
  toggleAriaLabel,
}: SplitButtonProps): React.JSX.Element {
  const [isOpen, setIsOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number }>({
    top: 0,
    left: 0,
  });
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuItemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const wasOpenRef = useRef(false);
  const menuId = useId();
  const triggerId = `${menuId}-trigger`;

  const hasDescriptions = menuItems.some((i) => Boolean(i.description));

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const triggerRect = triggerRef.current.getBoundingClientRect();
    const menuEl = menuRef.current;
    const menuWidth = menuEl?.offsetWidth || (hasDescriptions ? 270 : 260);
    const menuHeight = menuEl?.offsetHeight || 160;

    const spaceBelow = window.innerHeight - triggerRect.bottom;
    const spaceAbove = triggerRect.top;

    let top: number;
    if (spaceBelow < menuHeight + 8 && spaceAbove > spaceBelow) {
      top = Math.max(8, triggerRect.top - menuHeight - 4);
    } else {
      top = Math.max(8, Math.min(triggerRect.bottom + 4, window.innerHeight - menuHeight - 8));
    }

    let left = triggerRect.right - menuWidth;
    left = Math.max(8, Math.min(left, window.innerWidth - menuWidth - 8));

    setMenuPosition({ top, left });
  }, [hasDescriptions]);

  useIsomorphicLayoutEffect(() => {
    if (!isOpen) return undefined;
    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [isOpen, updatePosition]);

  useEffect(() => {
    if (isOpen && !wasOpenRef.current) {
      const firstEnabledIndex = menuItems.findIndex((item) => !item.disabled);
      if (firstEnabledIndex !== -1) {
        menuItemRefs.current[firstEnabledIndex]?.focus();
      }
    }
    wasOpenRef.current = isOpen;
  }, [isOpen, menuItems]);

  useEffect(() => {
    if (disabled && isOpen) {
      setIsOpen(false);
    }
  }, [disabled, isOpen]);

  useEffect(() => {
    if (!isOpen) return undefined;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        containerRef.current &&
        !containerRef.current.contains(target) &&
        menuRef.current &&
        !menuRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        setIsOpen(false);
        triggerRef.current?.focus();
        return;
      }

      if (event.key === 'Tab') {
        setIsOpen(false);
        return;
      }

      const isTargetInMenuOrContainer =
        Boolean(containerRef.current?.contains(event.target as Node)) ||
        Boolean(menuRef.current?.contains(event.target as Node));
      const isActiveElementInMenuOrContainer =
        Boolean(containerRef.current?.contains(document.activeElement)) ||
        Boolean(menuRef.current?.contains(document.activeElement));

      if (!isTargetInMenuOrContainer && !isActiveElementInMenuOrContainer) {
        return;
      }

      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const enabledIndices = menuItems
          .map((item, idx) => (!item.disabled ? idx : -1))
          .filter((idx) => idx !== -1);
        if (enabledIndices.length === 0) return;

        const activeEl = document.activeElement;
        const currentPos = enabledIndices.findIndex(
          (idx) => menuItemRefs.current[idx] === activeEl
        );

        let nextIndex: number;
        if (event.key === 'ArrowDown') {
          nextIndex =
            currentPos === -1 || currentPos === enabledIndices.length - 1
              ? enabledIndices[0]
              : enabledIndices[currentPos + 1];
        } else {
          nextIndex =
            currentPos <= 0
              ? enabledIndices[enabledIndices.length - 1]
              : enabledIndices[currentPos - 1];
        }
        menuItemRefs.current[nextIndex]?.focus();
        return;
      }

      if (event.key === 'Home') {
        event.preventDefault();
        const first = menuItems.findIndex((item) => !item.disabled);
        if (first !== -1) menuItemRefs.current[first]?.focus();
        return;
      }

      if (event.key === 'End') {
        event.preventDefault();
        for (let i = menuItems.length - 1; i >= 0; i--) {
          if (!menuItems[i].disabled) {
            menuItemRefs.current[i]?.focus();
            break;
          }
        }
        return;
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, menuItems]);

  const isMovingInside = (related: Node | null) => {
    if (!related) return false;
    return Boolean(containerRef.current?.contains(related) || menuRef.current?.contains(related));
  };

  const handleToggle = () => {
    if (!isOpen) {
      updatePosition();
    }
    setIsOpen((prev) => !prev);
  };

  const baseBtnClass = variant === 'primary' ? 'btn btn-primary' : 'btn btn-secondary-soft';
  const sizeClass = size === 'sm' ? 'btn-sm' : '';

  return (
    <div
      ref={containerRef}
      className={`split-button-container ${variant === 'secondary' ? 'is-secondary' : ''} ${className}`.trim()}
      onBlur={(e) => {
        if (e.relatedTarget && !isMovingInside(e.relatedTarget as Node)) {
          setIsOpen(false);
        }
      }}
    >
      <button
        type="button"
        className={`split-button-primary ${baseBtnClass} ${sizeClass}`.trim()}
        onClick={onPrimaryClick}
        disabled={disabled}
        aria-label={ariaLabel}
      >
        {primaryIcon}
        <span>{primaryLabel}</span>
      </button>

      <button
        ref={triggerRef}
        id={triggerId}
        type="button"
        className={`split-button-toggle ${baseBtnClass} ${sizeClass}`.trim()}
        onClick={handleToggle}
        disabled={disabled || menuItems.length === 0}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            updatePosition();
            setIsOpen(true);
          }
        }}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={isOpen ? menuId : undefined}
        aria-label={
          toggleAriaLabel ??
          (ariaLabel
            ? `${ariaLabel} options`
            : t('batch.more_options', { defaultValue: 'More options' }))
        }
      >
        <ChevronDownIcon width={12} height={12} aria-hidden="true" />
      </button>

      {isOpen && !disabled && (
        <ModalPortal>
          <div
            ref={menuRef}
            id={menuId}
            role="menu"
            aria-labelledby={triggerId}
            className={`split-button-portal-menu split-button-menu ${hasDescriptions ? 'has-descriptions' : ''} ${className}`.trim()}
            style={{
              top: `${menuPosition.top}px`,
              left: `${menuPosition.left}px`,
            }}
            onBlur={(e) => {
              if (e.relatedTarget && !isMovingInside(e.relatedTarget as Node)) {
                setIsOpen(false);
              }
            }}
          >
            {menuItems.map((item, index) => (
              <button
                key={item.key}
                ref={(el) => {
                  menuItemRefs.current[index] = el;
                }}
                type="button"
                role="menuitem"
                tabIndex={-1}
                disabled={item.disabled}
                onClick={() => {
                  setIsOpen(false);
                  item.onClick();
                }}
                className={`split-button-item ${item.description ? 'has-description' : ''}`.trim()}
              >
                {item.icon &&
                  (item.description ? (
                    <span className="split-button-item-icon-box" aria-hidden="true">
                      {item.icon}
                    </span>
                  ) : (
                    <span className="split-button-item-icon" aria-hidden="true">
                      {item.icon}
                    </span>
                  ))}
                <div className="split-button-item-content">
                  <span className="split-button-item-title">{item.label}</span>
                  {item.description && (
                    <span className="split-button-item-desc">{item.description}</span>
                  )}
                </div>
              </button>
            ))}
          </div>
        </ModalPortal>
      )}
    </div>
  );
}
