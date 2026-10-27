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
}: SplitButtonProps): React.JSX.Element {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();

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
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  const baseBtnClass = variant === 'primary' ? 'btn btn-primary' : 'btn btn-secondary-soft';
  const sizeClass = size === 'sm' ? 'btn-sm' : '';

  return (
    <div
      ref={containerRef}
      className={`split-button-container ${className}`}
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
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={menuId}
        aria-label={ariaLabel ? `${ariaLabel} options` : 'More options'}
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
          {menuItems.map((item) => (
            <button
              key={item.key}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              onClick={() => {
                setIsOpen(false);
                item.onClick();
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
