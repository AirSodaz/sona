import type React from 'react';
import type { SettingsTab } from '../../types/settings';

/** Props for the SettingsTabButton component. */
export interface SettingsTabButtonProps {
  id: SettingsTab;
  label: string;
  Icon: () => React.JSX.Element;
  activeTab: SettingsTab;
  setActiveTab: (id: SettingsTab) => void;
  /** Optional tabIndex for keyboard navigation management. */
  tabIndex?: number;
}

/**
 * A tab button for the settings sidebar.
 *
 * @param props Component props.
 * @return The rendered tab button.
 */
export function SettingsTabButton({
  id,
  label,
  Icon,
  activeTab,
  setActiveTab,
  tabIndex,
}: SettingsTabButtonProps): React.JSX.Element {
  return (
    <button
      className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
      onClick={() => {
        if (activeTab === id) {
          document
            .querySelector('.settings-content-scroll')
            ?.scrollTo({ top: 0, behavior: 'smooth' });
          return;
        }
        setActiveTab(id);
      }}
      role="tab"
      aria-label={label}
      aria-selected={activeTab === id}
      aria-controls={`settings-panel-${id}`}
      id={`settings-tab-${id}`}
      tabIndex={tabIndex}
    >
      <Icon />
      <span className="settings-tab-btn-text">{label}</span>
    </button>
  );
}

export function SettingsSidebarGroupHeader({
  label,
  first,
}: {
  label: string;
  first: boolean;
}): React.JSX.Element {
  return (
    <div
      className={`settings-sidebar-group-header${first ? '' : ' settings-sidebar-group-separator'}`}
      role="presentation"
    >
      <span className="settings-sidebar-group-label">{label}</span>
    </div>
  );
}
