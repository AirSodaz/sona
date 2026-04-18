import React from 'react';

/** Props for the SettingsTabButton component. */
export interface SettingsTabButtonProps {
    id: 'general' | 'microphone' | 'subtitle' | 'models' | 'local' | 'shortcuts' | 'about' | 'llm_service' | 'vocabulary';
    label: string;
    Icon: () => React.JSX.Element;
    activeTab: 'general' | 'microphone' | 'subtitle' | 'models' | 'local' | 'shortcuts' | 'about' | 'llm_service' | 'vocabulary';
    setActiveTab: (id: 'general' | 'microphone' | 'subtitle' | 'models' | 'local' | 'shortcuts' | 'about' | 'llm_service' | 'vocabulary') => void;
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
