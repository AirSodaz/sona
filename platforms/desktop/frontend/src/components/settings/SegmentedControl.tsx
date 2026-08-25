import React from 'react';

export interface SegmentedControlOption<T extends string> {
    value: T;
    label: React.ReactNode;
}

interface SegmentedControlProps<T extends string> {
    /** Accessible role label describing the group. */
    ariaLabel: string;
    id?: string;
    value: T;
    options: Array<SegmentedControlOption<T>>;
    onChange: (value: T) => void;
}

/**
 * A small segmented toggle for switching between exclusive modes,
 * styled after the sync panel's segmented control.
 */
export function SegmentedControl<T extends string>({
    ariaLabel,
    id,
    value,
    options,
    onChange,
}: SegmentedControlProps<T>): React.JSX.Element {
    return (
        <div id={id} className="settings-segmented-control" role="tablist" aria-label={ariaLabel}>
            {options.map((option) => (
                <button
                    key={option.value}
                    type="button"
                    role="tab"
                    aria-selected={option.value === value}
                    className={option.value === value ? 'active' : ''}
                    onClick={() => {
                        if (option.value !== value) {
                            onChange(option.value);
                        }
                    }}
                >
                    {option.label}
                </button>
            ))}
        </div>
    );
}
