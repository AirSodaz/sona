import React from 'react';

/** Props for the SettingsTabButton component. */
export interface SettingsTabButtonProps {
    id: 'general' | 'models' | 'local';
    label: string;
    Icon: () => React.JSX.Element;
    activeTab: string;
    setActiveTab: (id: 'general' | 'models' | 'local') => void;
}

/**
 * A tab button for the settings sidebar.
 *
 * @param props Component props.
 * @return The rendered tab button.
 */
export function SettingsTabButton({ id, label, Icon, activeTab, setActiveTab }: SettingsTabButtonProps): React.JSX.Element {
    return (
        <button
            className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
            onClick={() => setActiveTab(id)}
            role="tab"
            aria-selected={activeTab === id}
            aria-controls={`settings-panel-${id}`}
            id={`settings-tab-${id}`}
        >
            <Icon />
            {label}
        </button>
    );
}
