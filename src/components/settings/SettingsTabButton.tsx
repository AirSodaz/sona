import React from 'react';

/** Props for the SettingsTabButton component. */
export interface SettingsTabButtonProps {
    id: 'general' | 'models' | 'local' | 'shortcuts' | 'about';
    label: string;
    Icon: () => React.JSX.Element;
    activeTab: 'general' | 'models' | 'local' | 'shortcuts' | 'about';
    setActiveTab: (id: 'general' | 'models' | 'local' | 'shortcuts' | 'about') => void;
    /** Optional tabIndex for keyboard navigation management. */
    tabIndex?: number;
}

/**
 * A tab button for the settings sidebar.
 *
 * @param props Component props.
 * @return The rendered tab button.
 */
export function SettingsTabButton({ id, label, Icon, activeTab, setActiveTab, tabIndex }: SettingsTabButtonProps): React.JSX.Element {
    return (
        <button
            className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
            onClick={() => setActiveTab(id)}
            role="tab"
            aria-selected={activeTab === id}
            aria-controls={`settings-panel-${id}`}
            id={`settings-tab-${id}`}
            tabIndex={tabIndex}
        >
            <Icon />
            {label}
        </button>
    );
}
