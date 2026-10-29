import type React from 'react';
import { Switch } from '../Switch';

export type ToolHeroStatusTone = 'active' | 'warning' | 'inactive' | 'error';

export interface ToolHeroHeaderProps {
  id?: string;
  icon: React.ReactNode;
  title: string;
  description?: string;
  statusText: string;
  statusTone: ToolHeroStatusTone;
  hideSwitch?: boolean;
  enabled?: boolean;
  disabled?: boolean;
  onToggle?: (nextState: boolean) => void | Promise<void>;
  switchAriaLabel?: string;
  actions?: React.ReactNode;
}

export function ToolHeroHeader({
  id,
  icon,
  title,
  description,
  statusText,
  statusTone,
  hideSwitch = false,
  enabled = false,
  disabled,
  onToggle,
  switchAriaLabel = '',
  actions,
}: ToolHeroHeaderProps): React.JSX.Element {
  return (
    <div className="tool-hero-header" id={id}>
      <div className="tool-hero-identity">
        <div className="tool-hero-icon-tile" aria-hidden="true">
          {icon}
        </div>
        <div className="tool-hero-title-group">
          <h2 className="tool-hero-title">{title}</h2>
          {description && <p className="tool-hero-description">{description}</p>}
        </div>
      </div>
      <div className="tool-hero-control">
        {actions && <div className="tool-hero-actions">{actions}</div>}
        <div className="tool-hero-status">
          <span className={`tools-status-badge tools-status-${statusTone}`} role="status">
            <span className="tools-status-dot" aria-hidden="true" />
            <span>{statusText}</span>
          </span>
        </div>
        {!hideSwitch && (
          <Switch
            checked={enabled}
            disabled={disabled}
            onChange={(val) => {
              if (onToggle) {
                Promise.resolve(onToggle(val)).catch(() => {
                  // Rejection handled by caller
                });
              }
            }}
            aria-label={switchAriaLabel || title}
          />
        )}
      </div>
    </div>
  );
}
