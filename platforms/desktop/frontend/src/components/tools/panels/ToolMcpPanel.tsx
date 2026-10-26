import type React from 'react';
import { SettingsMcpTab } from '../../settings/SettingsMcpTab';

export function ToolMcpPanel(): React.JSX.Element {
  return (
    <div className="tool-embedded-panel">
      <SettingsMcpTab />
    </div>
  );
}
