interface SwitchProps {
    id?: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
    className?: string;
    disabled?: boolean;
    'aria-label'?: string;
    'aria-labelledby'?: string;
    style?: React.CSSProperties;
}

export function Switch({
    id,
    checked,
    onChange,
    label,
    className = '',
    disabled = false,
    'aria-label': ariaLabel,
    'aria-labelledby': ariaLabelledby,
    style,
}: SwitchProps) {
    const classNames = ['switch-container'];
    if (checked) classNames.push('checked');
    if (disabled) classNames.push('disabled');
    if (className) classNames.push(className);
    const accessibleLabel = ariaLabel || label;

    return (
        <div
            id={id}
            className={classNames.join(' ')}
            style={style}
            onClick={(e) => {
                if (disabled) return;
                e.preventDefault();
                onChange(!checked);
            }}
            role="switch"
            aria-checked={checked}
            aria-disabled={disabled}
            aria-label={accessibleLabel}
            aria-labelledby={accessibleLabel ? undefined : ariaLabelledby}
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
