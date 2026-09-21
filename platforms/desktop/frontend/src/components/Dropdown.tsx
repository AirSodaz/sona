import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ChevronDownIcon } from './Icons';
import { ModalPortal } from './ModalPortal';

export interface DropdownOption {
  value: string;
  label: React.ReactNode;
  description?: string;
  disabled?: boolean;
  ariaLabel?: string;
  style?: React.CSSProperties;
  /**
   * Display label of the section this option belongs to. Options sharing the
   * same group label render under one sticky-free heading; options without a
   * group render above grouped ones in list order.
   */
  group?: string;
}

interface DropdownProps {
  options: DropdownOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  style?: React.CSSProperties;
  id?: string;
  disabled?: boolean;
  'aria-label'?: string;
}

export function Dropdown({
  options,
  value,
  onChange,
  placeholder = 'Select...',
  className = '',
  style,
  id,
  disabled = false,
  'aria-label': ariaLabel,
}: DropdownProps): React.JSX.Element {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({});
  const [searchQuery, setSearchQuery] = useState('');
  const dropdownRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const initialTriggerRectRef = useRef<DOMRect | null>(null);

  const showSearch = options.length > 10;
  const selectedOption = options.find((opt) => opt.value === value);

  const calculatePosition = useCallback((): {
    position: 'bottom' | 'top';
    style: React.CSSProperties;
  } | null => {
    if (!dropdownRef.current) return null;
    const rect = dropdownRef.current.getBoundingClientRect();
    const menuMaxHeight = 280;
    const spaceBelow = window.innerHeight - rect.bottom;
    const spaceAbove = rect.top;

    let newPosition: 'bottom' | 'top' = 'bottom';
    if (spaceBelow < menuMaxHeight + 20 && spaceAbove > spaceBelow) {
      newPosition = 'top';
    }

    const style: React.CSSProperties = {
      position: 'fixed',
      left: rect.left,
      width: rect.width,
      zIndex: 2500,
    };

    if (newPosition === 'top') {
      style.bottom = window.innerHeight - rect.top + 4;
    } else {
      style.top = rect.bottom + 4;
    }

    return { position: newPosition, style };
  }, []);

  const updatePosition = useCallback(() => {
    const calculated = calculatePosition();
    if (calculated) {
      setPosition(calculated.position);
      setMenuStyle(calculated.style);
    }
  }, [calculatePosition]);

  const openMenu = useCallback(() => {
    if (disabled) return;
    setSearchQuery('');
    updatePosition();
    if (dropdownRef.current) {
      initialTriggerRectRef.current = dropdownRef.current.getBoundingClientRect();
    }
    setIsOpen(true);
  }, [disabled, updatePosition]);

  const closeMenu = useCallback(() => {
    setIsOpen(false);
    setSearchQuery('');
    initialTriggerRectRef.current = null;
  }, []);

  useEffect(() => {
    if (disabled) {
      const frameId = requestAnimationFrame(() => closeMenu());
      return () => cancelAnimationFrame(frameId);
    }
    return undefined;
  }, [disabled, closeMenu]);

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleClickOutside = (event: MouseEvent) => {
      const targetNode = event.target as Node | null;
      if (
        dropdownRef.current &&
        targetNode instanceof Node &&
        (dropdownRef.current.contains(targetNode) || menuRef.current?.contains(targetNode))
      ) {
        return;
      }
      closeMenu();
    };

    const handleScroll = (event: Event) => {
      const targetNode = event.target as Node | null;
      if (
        menuRef.current &&
        targetNode instanceof Node &&
        (menuRef.current === targetNode || menuRef.current.contains(targetNode))
      ) {
        return;
      }

      if (dropdownRef.current && initialTriggerRectRef.current) {
        const currentRect = dropdownRef.current.getBoundingClientRect();
        const hasMoved =
          Math.abs(currentRect.top - initialTriggerRectRef.current.top) > 1 ||
          Math.abs(currentRect.left - initialTriggerRectRef.current.left) > 1;
        if (!hasMoved) {
          return;
        }
      }

      closeMenu();
    };

    document.addEventListener('mousedown', handleClickOutside);
    window.addEventListener('scroll', handleScroll, true);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      window.removeEventListener('scroll', handleScroll, true);
    };
  }, [isOpen, closeMenu]);

  // Focus management when opening
  useEffect(() => {
    if (isOpen && menuRef.current) {
      if (showSearch) {
        const searchInput =
          menuRef.current.querySelector<HTMLInputElement>('.dropdown-search-input');
        if (searchInput) {
          requestAnimationFrame(() => searchInput.focus({ preventScroll: true }));
          return;
        }
      }
      // Try to find selected option first
      const selectedBtn = menuRef.current.querySelector<HTMLButtonElement>(
        '.selected:not(:disabled)'
      );
      if (selectedBtn) {
        requestAnimationFrame(() => selectedBtn.focus({ preventScroll: true }));
      } else {
        const firstButton = menuRef.current.querySelector<HTMLButtonElement>(
          '.dropdown-item:not(:disabled)'
        );
        if (firstButton) {
          requestAnimationFrame(() => firstButton.focus({ preventScroll: true }));
        }
      }
    }
  }, [isOpen, showSearch]);

  useLayoutEffect(() => {
    if (isOpen) {
      updatePosition();
      if (dropdownRef.current && !initialTriggerRectRef.current) {
        initialTriggerRectRef.current = dropdownRef.current.getBoundingClientRect();
      }
    } else {
      initialTriggerRectRef.current = null;
    }
  }, [isOpen, updatePosition]);

  const handleSelect = (option: DropdownOption) => {
    if (option.disabled) {
      return;
    }
    onChange(option.value);
    closeMenu();
    triggerRef.current?.focus({ preventScroll: true });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) {
      return;
    }

    if (e.key === 'Escape') {
      e.preventDefault();
      closeMenu();
      triggerRef.current?.focus({ preventScroll: true });
      return;
    }

    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        openMenu();
      }
      return;
    }

    if (menuRef.current) {
      const searchInput = menuRef.current.querySelector<HTMLInputElement>('.dropdown-search-input');
      const buttons = Array.from(
        menuRef.current.querySelectorAll<HTMLButtonElement>('.dropdown-item:not(:disabled)')
      );
      const activeElement = document.activeElement;

      if (searchInput && activeElement === searchInput) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          buttons[0]?.focus();
        }
        return;
      }

      const currentIndex = buttons.indexOf(activeElement as HTMLButtonElement);

      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          if (buttons.length === 0) return;
          if (currentIndex === buttons.length - 1) {
            if (searchInput) {
              searchInput.focus();
            } else {
              buttons[0]?.focus();
            }
          } else {
            buttons[currentIndex + 1]?.focus();
          }
          break;

        case 'ArrowUp':
          e.preventDefault();
          if (buttons.length === 0) return;
          if (currentIndex === 0 || currentIndex === -1) {
            if (searchInput) {
              searchInput.focus();
            } else {
              buttons[buttons.length - 1]?.focus();
            }
          } else {
            buttons[currentIndex - 1]?.focus();
          }
          break;

        case 'Home':
          e.preventDefault();
          buttons[0]?.focus();
          break;

        case 'End':
          e.preventDefault();
          buttons[buttons.length - 1]?.focus();
          break;

        case 'Tab':
          closeMenu();
          break;

        default: {
          // Standard key redirects typing back to search input
          const isAlphanumeric = /^[a-zA-Z0-9]$/.test(e.key);
          if (searchInput && isAlphanumeric && !e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            searchInput.focus();
            setSearchQuery((prev) => prev + e.key);
          }
          break;
        }
      }
    }
  };

  const filteredOptions = showSearch
    ? options.filter((opt) => {
        const labelText = typeof opt.label === 'string' ? opt.label : (opt.ariaLabel ?? '');
        return (
          labelText.toLowerCase().includes(searchQuery.toLowerCase()) ||
          opt.value.toLowerCase().includes(searchQuery.toLowerCase())
        );
      })
    : options;

  return (
    <div
      className={`dropdown-container ${className}`}
      ref={dropdownRef}
      style={style}
      onKeyDown={handleKeyDown}
    >
      <button
        ref={triggerRef}
        type="button"
        id={id}
        className={`dropdown-trigger ${isOpen ? 'active' : ''} ${disabled ? 'disabled' : ''}`}
        onClick={() => {
          if (disabled) {
            return;
          }
          if (isOpen) {
            closeMenu();
          } else {
            openMenu();
          }
        }}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-label={ariaLabel}
      >
        <span className="dropdown-value">
          {selectedOption ? selectedOption.label : placeholder}
        </span>
        <ChevronDownIcon className="dropdown-icon" />
      </button>

      {isOpen && (
        <ModalPortal>
          <div
            ref={menuRef}
            className={`dropdown-menu position-${position}`}
            role="listbox"
            style={{
              position: 'fixed',
              visibility: menuStyle.left !== undefined ? 'visible' : 'hidden',
              ...menuStyle,
            }}
            onKeyDown={(e) => {
              e.stopPropagation();
              handleKeyDown(e);
            }}
          >
            {showSearch && (
              <div
                className="dropdown-search-wrapper"
                style={{
                  padding: '8px',
                  borderBottom: '1px solid var(--border)',
                  background: 'var(--bg-secondary)',
                  position: 'sticky',
                  top: 0,
                  zIndex: 10,
                }}
              >
                <input
                  type="text"
                  className="dropdown-search-input"
                  placeholder="Search..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '6px 8px',
                    border: '1px solid var(--border)',
                    borderRadius: '4px',
                    fontSize: '0.85rem',
                    outline: 'none',
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                  }}
                  onClick={(e) => e.stopPropagation()} // Prevent clicking search from triggering select/close
                />
              </div>
            )}
            {(() => {
              let lastGroup: string | undefined = '';
              return filteredOptions.map((option) => {
                const header =
                  option.group && option.group !== lastGroup ? (
                    <div key={`group-${option.group}`} className="dropdown-group-header">
                      {option.group}
                    </div>
                  ) : null;
                lastGroup = option.group;
                return (
                  <React.Fragment key={option.value}>
                    {header}
                    <button
                      type="button"
                      className={`dropdown-item ${option.value === value ? 'selected' : ''}`}
                      onClick={() => handleSelect(option)}
                      role="option"
                      aria-selected={option.value === value}
                      aria-label={option.ariaLabel}
                      aria-disabled={option.disabled || undefined}
                      disabled={option.disabled}
                      tabIndex={-1}
                      style={option.style}
                      title={option.description}
                    >
                      <span className="dropdown-item-content">
                        <span>{option.label}</span>
                        {option.description && (
                          <span className="dropdown-item-description">{option.description}</span>
                        )}
                      </span>
                    </button>
                  </React.Fragment>
                );
              });
            })()}
          </div>
        </ModalPortal>
      )}
    </div>
  );
}
