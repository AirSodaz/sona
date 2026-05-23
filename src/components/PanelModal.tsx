import React from 'react';
import { X } from 'lucide-react';

import './PanelModal.css';

interface PanelModalProps {
  isOpen: boolean;
  onClose: () => void;
  ariaLabel?: string;
  ariaLabelledby?: string;
  className?: string;
  overlayClassName?: string;
  headerClassName?: string;
  headerCopyClassName?: string;
  headerControlsClassName?: string;
  toolbarClassName?: string;
  badgeClassName?: string;
  metaClassName?: string;
  contentClassName?: string;
  badge?: React.ReactNode;
  title: React.ReactNode;
  description?: React.ReactNode;
  headerActions?: React.ReactNode;
  meta?: React.ReactNode;
  errorBanner?: React.ReactNode;
  children: React.ReactNode;
  shellRef?: React.Ref<HTMLDivElement>;
}

function joinClassNames(...parts: Array<string | undefined | false | null>): string {
  return parts.filter(Boolean).join(' ');
}

export function PanelModal({
  isOpen,
  onClose,
  ariaLabel,
  ariaLabelledby,
  className,
  overlayClassName,
  headerClassName,
  headerCopyClassName,
  headerControlsClassName,
  toolbarClassName,
  badgeClassName,
  metaClassName,
  contentClassName,
  badge,
  title,
  description,
  headerActions,
  meta,
  errorBanner,
  children,
  shellRef,
}: PanelModalProps): React.JSX.Element | null {
  if (!isOpen) {
    return null;
  }

  return (
    <div
      className={joinClassNames('settings-overlay', 'panel-modal-overlay', overlayClassName)}
      onClick={onClose}
    >
      <div
        ref={shellRef}
        className={joinClassNames('panel-modal-shell', className)}
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledby}
      >
        <div className={joinClassNames('panel-modal-header', headerClassName)}>
          <div className={joinClassNames('panel-modal-header-copy', headerCopyClassName)}>
            {badge ? (
              <div className={joinClassNames('panel-modal-badge', badgeClassName)}>
                {badge}
              </div>
            ) : null}
            {typeof title === 'string'
              ? <h2 id={ariaLabelledby}>{title}</h2>
              : title}
            {description ? <p>{description}</p> : null}
          </div>
          <div className={joinClassNames('panel-modal-header-controls', headerControlsClassName)}>
            {headerActions ? (
              <div className={joinClassNames('panel-modal-toolbar', toolbarClassName)}>
                {headerActions}
              </div>
            ) : null}
            <button
              type="button"
              className="btn btn-icon panel-modal-close"
              onClick={onClose}
              aria-label="Close"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {meta ? (
          <div className={joinClassNames('panel-modal-meta-row', metaClassName)}>
            {meta}
          </div>
        ) : null}

        {errorBanner}

        <div className={joinClassNames('panel-modal-content', contentClassName)}>
          {children}
        </div>
      </div>
    </div>
  );
}
