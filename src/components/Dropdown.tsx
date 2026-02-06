import React, { useState, useRef, useEffect } from 'react';
import { ChevronDownIcon } from './Icons';

export interface DropdownOption {
    value: string;
    label: string;
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
    'aria-label': ariaLabel
}: DropdownProps): React.JSX.Element {
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const dropdownRef = useRef<HTMLDivElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);

    const selectedOption = options.find(opt => opt.value === value);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // Focus management when opening
    useEffect(() => {
        if (isOpen && menuRef.current) {
            // Try to find selected option first
            const selectedBtn = menuRef.current.querySelector('.selected') as HTMLElement;
            if (selectedBtn) {
                requestAnimationFrame(() => selectedBtn.focus());
            } else {
                const firstButton = menuRef.current.querySelector('button');
                if (firstButton) {
                    requestAnimationFrame(() => firstButton.focus());
                }
            }
        }
    }, [isOpen]);

    // Reset position when closing
    useEffect(() => {
        if (!isOpen) {
            setPosition('bottom');
        }
    }, [isOpen]);

    React.useLayoutEffect(() => {
        if (isOpen && dropdownRef.current && menuRef.current) {
            const rect = dropdownRef.current.getBoundingClientRect();
            const menuHeight = menuRef.current.offsetHeight;
            const spaceBelow = window.innerHeight - rect.bottom;

            // buffer for visuals (e.g. shadow, margin)
            const buffer = 10;

            if (spaceBelow < (menuHeight + buffer) && rect.top > (menuHeight + buffer)) {
                setPosition('top');
            } else {
                setPosition('bottom');
            }
        }
    }, [isOpen]);

    const handleSelect = (optionValue: string) => {
        onChange(optionValue);
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
            if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                setIsOpen(true);
            }
            return;
        }

        if (menuRef.current) {
            const buttons = Array.from(menuRef.current.querySelectorAll('button'));
            const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                const nextIndex = (currentIndex + 1) % buttons.length;
                buttons[nextIndex].focus();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                const prevIndex = (currentIndex - 1 + buttons.length) % buttons.length;
                buttons[prevIndex].focus();
            } else if (e.key === 'Home') {
                e.preventDefault();
                buttons[0].focus();
            } else if (e.key === 'End') {
                e.preventDefault();
                buttons[buttons.length - 1].focus();
            } else if (e.key === 'Tab') {
                // Close on tab (default behavior of tabbing away)
                setIsOpen(false);
            }
        }
    };

    const handleBlur = (e: React.FocusEvent) => {
        // Close menu if focus leaves the component
        if (!dropdownRef.current?.contains(e.relatedTarget as Node)) {
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
                <span className="dropdown-value">
                    {selectedOption ? selectedOption.label : placeholder}
                </span>
                <ChevronDownIcon className="dropdown-icon" />
            </button>

            {isOpen && (
                <div
                    ref={menuRef}
                    className={`dropdown-menu position-${position}`}
                    role="listbox"
                >
                    {options.map((option) => (
                        <button
                            key={option.value}
                            type="button"
                            className={`dropdown-item ${option.value === value ? 'selected' : ''}`}
                            onClick={() => handleSelect(option.value)}
                            role="option"
                            aria-selected={option.value === value}
                            tabIndex={-1}
                            style={option.style}
                        >
                            {option.label}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
