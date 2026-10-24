import type React from 'react';
import { SettingsApiServerTab } from '../../settings/SettingsApiServerTab';

export function ToolApiServerPanel(): React.JSX.Element {
  return (
    <div className="tool-embedded-panel">
      <SettingsApiServerTab />
    </div>
  );
}
