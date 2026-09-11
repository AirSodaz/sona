import React from 'react';
import { FolderIcon } from '../Icons';
import { SYSTEM_ICONS } from '../IconPicker';

export const DEFAULT_PROJECT_COLOR = '#64748B';

export interface ProjectVisualProps {
  icon?: string | null;
  color?: string | null;
  size?: 'xs' | 'sm' | 'md' | 'lg';
  showBackground?: boolean;
  className?: string;
  style?: React.CSSProperties;
  defaultIcon?: React.ReactNode;
}

export function ProjectVisual({
  icon,
  color,
  size = 'sm',
  showBackground = false,
  className = '',
  style,
  defaultIcon,
}: ProjectVisualProps): React.JSX.Element {
  const resolvedColor = color || DEFAULT_PROJECT_COLOR;
  const isSystemIcon = Boolean(icon?.startsWith('system:'));
  const isEmoji = Boolean(icon && !isSystemIcon);

  const containerStyle: React.CSSProperties = {
    ...style,
    '--project-visual-color': resolvedColor,
  } as React.CSSProperties;

  const resolvedClassName = [
    'project-visual',
    `project-visual--${size}`,
    showBackground ? 'project-visual--with-bg' : '',
    className,
  ]
    .filter(Boolean)
    .join(' ');

  if (isEmoji) {
    return (
      <span className={resolvedClassName} style={containerStyle} aria-hidden="true">
        <span className="project-visual-emoji">{icon}</span>
      </span>
    );
  }

  if (isSystemIcon) {
    const sysIcon = SYSTEM_ICONS.find((si) => si.id === icon);
    return (
      <span className={resolvedClassName} style={containerStyle} aria-hidden="true">
        <span className="project-visual-svg" style={{ color: resolvedColor }}>
          {sysIcon ? sysIcon.icon : defaultIcon || <FolderIcon />}
        </span>
      </span>
    );
  }

  return (
    <span className={resolvedClassName} style={containerStyle} aria-hidden="true">
      <span className="project-visual-svg" style={{ color: resolvedColor }}>
        {defaultIcon || <FolderIcon />}
      </span>
    </span>
  );
}

export interface ProjectBadgeProps {
  name: string;
  icon?: string | null;
  color?: string | null;
  className?: string;
  style?: React.CSSProperties;
}

export function ProjectBadge({
  name,
  icon,
  color,
  className = '',
  style,
}: ProjectBadgeProps): React.JSX.Element {
  const resolvedColor = color || DEFAULT_PROJECT_COLOR;

  const badgeStyle: React.CSSProperties = {
    ...style,
    '--project-color': resolvedColor,
  } as React.CSSProperties;

  return (
    <span className={`history-item-project-badge ${className}`.trim()} style={badgeStyle}>
      <ProjectVisual icon={icon} color={resolvedColor} size="xs" showBackground={false} />
      <span className="history-item-project-badge-text">{name}</span>
    </span>
  );
}
