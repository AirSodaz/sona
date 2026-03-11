

interface SwitchProps {
    checked: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
    className?: string;
    disabled?: boolean;
}

export function Switch({ checked, onChange, label, className = '', disabled = false }: SwitchProps) {
    const classNames = ['switch-container'];
    if (checked) classNames.push('checked');
    if (disabled) classNames.push('disabled');
    if (className) classNames.push(className);

    return (
        <div
            className={classNames.join(' ')}
            onClick={(e) => {
                if (disabled) return;
                e.preventDefault();
                onChange(!checked);
            }}
            role="switch"
            aria-checked={checked}
            aria-disabled={disabled}
            tabIndex={disabled ? -1 : 0}
            onKeyDown={(e) => {
                if (disabled) return;
                if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onChange(!checked);
                }
            }}
        >
            <div className="switch-track">
                <div className="switch-thumb" />
            </div>
            {label && (
                <span className="switch-label">
                    {label}
                </span>
            )}
        </div>
    );
}
