import { MinusSquare, Square, CheckSquare } from 'lucide-react';

interface CheckboxProps {
    checked: boolean;
    indeterminate?: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
    className?: string;
    'aria-label'?: string;
}

export function Checkbox({ checked, indeterminate = false, onChange, label, className = '', 'aria-label': ariaLabel }: CheckboxProps) {
    return (
        <div
            className={`checkbox-container ${className}`}
            style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--spacing-sm)',
                cursor: 'pointer',
                userSelect: 'none'
            }}
            role="checkbox"
            aria-checked={indeterminate ? 'mixed' : checked}
            aria-label={ariaLabel || label}
            tabIndex={0}
            onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onChange(indeterminate || !checked);
            }}
            onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    e.stopPropagation();
                    onChange(indeterminate || !checked);
                }
            }}
        >
            <div
                className="checkbox-icon"
                style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: checked ? 'var(--color-primary, #37352f)' : 'var(--color-text-muted)',
                    transition: 'color var(--transition-fast)'
                }}
            >
                {indeterminate ? (
                    <MinusSquare size={18} strokeWidth={2.5} />
                ) : checked ? (
                    <CheckSquare size={18} strokeWidth={2.5} />
                ) : (
                    <Square size={18} strokeWidth={2.5} />
                )}
            </div>
            {label && (
                <span style={{
                    fontSize: '0.875rem',
                    color: 'var(--color-text-primary)'
                }}>
                    {label}
                </span>
            )}
        </div>
    );
}
