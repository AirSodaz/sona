import React from 'react';
import { FolderOpen } from 'lucide-react';

export interface EmptyStateProps {
  title: string;
  description?: string;
  icon?: React.ReactNode;
  actionLabel?: string;
  onAction?: () => void;
  className?: string;
}

export function EmptyState({
  title,
  description,
  icon,
  actionLabel,
  onAction,
  className = '',
}: EmptyStateProps): React.JSX.Element {
  const resolvedIcon = icon ?? <FolderOpen size={24} />;

  return (
    <div className={`shared-empty-container ${className}`}>
      {/* Icon wraps */}
      <div className="shared-empty-icon-wrap">
        {resolvedIcon}
      </div>

      {/* Info labels */}
      <div className="shared-empty-info">
        <h4 className="shared-empty-title">{title}</h4>
        {description && (
          <p className="shared-empty-desc">{description}</p>
        )}
      </div>

      {/* Action button */}
      {actionLabel && onAction && (
        <div className="shared-empty-action">
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={onAction}
          >
            {actionLabel}
          </button>
        </div>
      )}
    </div>
  );
}
