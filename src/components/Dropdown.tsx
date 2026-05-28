import React, { useState, useRef, useEffect } from 'react';
import { ChevronDownIcon } from './Icons';
import { ModalPortal } from './ModalPortal';

export interface DropdownOption {
    value: string;
    label: React.ReactNode;
    description?: string;
    disabled?: boolean;
    ariaLabel?: string;
    style?: React.CSSProperties;
}

interface DropdownProps {
    options: DropdownOption[];
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    className?: string;
    style?: React.CSSProperties;
    id?: string;
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
    'aria-label': ariaLabel,
}: DropdownProps): React.JSX.Element {
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({});
    const dropdownRef = useRef<HTMLDivElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);

    const selectedOption = options.find((opt) => opt.value === value);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node) && !menuRef.current?.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        const handleScroll = () => {
            setIsOpen(false);
        };

        document.addEventListener('mousedown', handleClickOutside);
        window.addEventListener('scroll', handleScroll, true); // Capture to catch any scroll
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
            window.removeEventListener('scroll', handleScroll, true);
        };
    }, []);

    // Focus management when opening
    useEffect(() => {
        if (isOpen && menuRef.current) {
            // Try to find selected option first
            const selectedBtn = menuRef.current.querySelector<HTMLButtonElement>('.selected:not(:disabled)');
            if (selectedBtn) {
                requestAnimationFrame(() => selectedBtn.focus());
            } else {
                const firstButton = menuRef.current.querySelector<HTMLButtonElement>('button:not(:disabled)');
                if (firstButton) {
                    requestAnimationFrame(() => firstButton.focus());
                }
            }
        }
    }, [isOpen]);

    React.useLayoutEffect(() => {
        if (isOpen && dropdownRef.current) {
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

            setPosition(newPosition);
            setMenuStyle(style);
        }
    }, [isOpen]);

    const handleSelect = (option: DropdownOption) => {
        if (option.disabled) {
            return;
        }
        onChange(option.value);
        setIsOpen(false);
        triggerRef.current?.focus();
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Escape') {
            e.preventDefault();
            setIsOpen(false);
            triggerRef.current?.focus();
            return;
        }

        // If not open, Open on ArrowDown/Enter/Space
        if (!isOpen) {
            switch (e.key) {
                case 'ArrowDown':
                case 'Enter':
                case ' ':
                    e.preventDefault();
                    setIsOpen(true);
                    break;
                default:
                    break;
            }
            return;
        }

        if (menuRef.current) {
            const buttons = Array.from(menuRef.current.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));
            if (buttons.length === 0) {
                return;
            }
            const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);

            switch (e.key) {
                case 'ArrowDown': {
                    e.preventDefault();
                    const nextIndex = (currentIndex + 1) % buttons.length;
                    buttons[nextIndex].focus();
                    break;
                }
                case 'ArrowUp': {
                    e.preventDefault();
                    const prevIndex = (currentIndex - 1 + buttons.length) % buttons.length;
                    buttons[prevIndex].focus();
                    break;
                }
                case 'Home':
                    e.preventDefault();
                    buttons[0].focus();
                    break;
                case 'End':
                    e.preventDefault();
                    buttons[buttons.length - 1].focus();
                    break;
                case 'Tab':
                    // Close on tab (default behavior of tabbing away)
                    setIsOpen(false);
                    break;
                default:
                    break;
            }
        }
    };

    const handleBlur = (e: React.FocusEvent) => {
        // Close menu if focus leaves the component (either trigger or portal menu)
        if (
            !dropdownRef.current?.contains(e.relatedTarget as Node) &&
            !menuRef.current?.contains(e.relatedTarget as Node)
        ) {
            setIsOpen(false);
        }
    };

    return (
        <div
            className={`dropdown-container ${className}`}
            ref={dropdownRef}
            style={style}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
        >
            <button
                ref={triggerRef}
                type="button"
                id={id}
                className={`dropdown-trigger ${isOpen ? 'active' : ''}`}
                onClick={() => setIsOpen(!isOpen)}
                aria-haspopup="listbox"
                aria-expanded={isOpen}
                aria-label={ariaLabel}
            >
                <span className="dropdown-value">{selectedOption ? selectedOption.label : placeholder}</span>
                <ChevronDownIcon className="dropdown-icon" />
            </button>

            {isOpen && (
                <ModalPortal>
                    <div
                        ref={menuRef}
                        className={`dropdown-menu position-${position}`}
                        role="listbox"
                        style={menuStyle}
                    >
                        {options.map((option) => (
                            <button
                                key={option.value}
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
                        ))}
                    </div>
                </ModalPortal>
            )}
        </div>
    );
}
