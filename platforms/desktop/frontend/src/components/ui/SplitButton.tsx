import type React from 'react';
import { useEffect, useId, useRef, useState } from 'react';
import { ChevronDownIcon } from '../Icons';

export interface SplitButtonItem {
  key: string;
  label: React.ReactNode;
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
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuItemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const menuId = useId();

  useEffect(() => {
    if (isOpen) {
      const firstEnabledIndex = menuItems.findIndex((item) => !item.disabled);
      if (firstEnabledIndex !== -1) {
        menuItemRefs.current[firstEnabledIndex]?.focus();
      }
    }
  }, [isOpen, menuItems]);

  useEffect(() => {
    if (!isOpen) return undefined;
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
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

  const baseBtnClass = variant === 'primary' ? 'btn btn-primary' : 'btn btn-secondary-soft';
  const sizeClass = size === 'sm' ? 'btn-sm' : '';

  return (
    <div
      ref={containerRef}
      className={`split-button-container ${className}`}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
          setIsOpen(false);
        }
      }}
      style={{
        display: 'inline-flex',
        alignItems: 'stretch',
        position: 'relative',
        borderRadius: 'var(--radius-md, 6px)',
      }}
    >
      <button
        type="button"
        className={`${baseBtnClass} ${sizeClass}`}
        onClick={onPrimaryClick}
        disabled={disabled}
        aria-label={ariaLabel}
        style={{
          borderTopRightRadius: 0,
          borderBottomRightRadius: 0,
          display: 'inline-flex',
          alignItems: 'center',
          gap: '6px',
        }}
      >
        {primaryIcon}
        <span>{primaryLabel}</span>
      </button>

      <button
        ref={triggerRef}
        type="button"
        className={`${baseBtnClass} ${sizeClass}`}
        onClick={() => setIsOpen((prev) => !prev)}
        disabled={disabled || menuItems.length === 0}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            setIsOpen(true);
          }
        }}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={isOpen ? menuId : undefined}
        aria-label={toggleAriaLabel ?? (ariaLabel ? `${ariaLabel} options` : 'More options')}
        style={{
          borderTopLeftRadius: 0,
          borderBottomLeftRadius: 0,
          borderLeft: '1px solid rgba(255, 255, 255, 0.2)',
          paddingLeft: '6px',
          paddingRight: '6px',
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <ChevronDownIcon width={12} height={12} aria-hidden="true" />
      </button>

      {isOpen && (
        <div
          id={menuId}
          role="menu"
          style={{
            position: 'absolute',
            top: 'calc(100% + 4px)',
            right: 0,
            zIndex: 1000,
            minWidth: '180px',
            backgroundColor: 'var(--color-bg-surface, #1e1e24)',
            border: '1px solid var(--color-border, #333)',
            borderRadius: 'var(--radius-md, 6px)',
            boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)',
            padding: '4px',
            display: 'flex',
            flexDirection: 'column',
            gap: '2px',
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
              disabled={item.disabled}
              onClick={() => {
                setIsOpen(false);
                item.onClick();
              }}
              onFocus={(e) => {
                if (!item.disabled) {
                  e.currentTarget.style.backgroundColor =
                    'var(--color-bg-hover, rgba(255, 255, 255, 0.08))';
                }
              }}
              onBlur={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                width: '100%',
                padding: '8px 10px',
                fontSize: '13px',
                textAlign: 'left',
                border: 'none',
                background: 'transparent',
                color: 'var(--color-text, #eee)',
                borderRadius: 'var(--radius-sm, 4px)',
                cursor: item.disabled ? 'not-allowed' : 'pointer',
                opacity: item.disabled ? 0.5 : 1,
              }}
              onMouseEnter={(e) => {
                if (!item.disabled) {
                  e.currentTarget.style.backgroundColor =
                    'var(--color-bg-hover, rgba(255, 255, 255, 0.08))';
                }
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
            >
              {item.icon && (
                <span style={{ display: 'inline-flex', alignItems: 'center' }}>{item.icon}</span>
              )}
              <span>{item.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
