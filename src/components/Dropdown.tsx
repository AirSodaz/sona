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
}

export function Dropdown({
    options,
    value,
    onChange,
    placeholder = 'Select...',
    className = '',
    style,
    id
}: DropdownProps): React.JSX.Element {
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const dropdownRef = useRef<HTMLDivElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

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
    };

    return (
        <div
            className={`dropdown-container ${className}`}
            ref={dropdownRef}
            style={style}
            id={id}
        >
            <button
                type="button"
                className={`dropdown-trigger ${isOpen ? 'active' : ''}`}
                onClick={() => setIsOpen(!isOpen)}
                aria-haspopup="listbox"
                aria-expanded={isOpen}
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
