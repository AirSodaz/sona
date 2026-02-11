import { Square, CheckSquare } from 'lucide-react';

interface CheckboxProps {
    checked: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
    className?: string;
}

export function Checkbox({ checked, onChange, label, className = '' }: CheckboxProps) {
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
            onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onChange(!checked);
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
                {checked ? (
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
