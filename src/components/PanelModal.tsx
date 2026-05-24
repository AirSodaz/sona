import React from 'react';
import { ArrowLeft, X } from 'lucide-react';

import './PanelModal.css';

type PanelModalSize = 'default' | 'settings';
type PanelModalOrigin = 'standalone' | 'settings';

interface PanelModalProps {
  isOpen: boolean;
  onClose: () => void;
  ariaLabel?: string;
  ariaLabelledby?: string;
  size?: PanelModalSize;
  origin?: PanelModalOrigin;
  onBack?: () => void;
  backLabel?: string;
  className?: string;
  overlayClassName?: string;
  headerClassName?: string;
  headerCopyClassName?: string;
  headerControlsClassName?: string;
  toolbarClassName?: string;
  badgeClassName?: string;
  metaClassName?: string;
  contentClassName?: string;
  headerLeading?: React.ReactNode;
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
  size = 'default',
  origin = 'standalone',
  onBack,
  backLabel = 'Back',
  className,
  overlayClassName,
  headerClassName,
  headerCopyClassName,
  headerControlsClassName,
  toolbarClassName,
  badgeClassName,
  metaClassName,
  contentClassName,
  headerLeading,
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
      className={joinClassNames(
        'panel-modal-overlay',
        `panel-modal-origin-${origin}`,
        overlayClassName,
      )}
      onClick={onClose}
    >
      <div
        ref={shellRef}
        className={joinClassNames('panel-modal-shell', `panel-modal-size-${size}`, className)}
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledby}
      >
        <div className={joinClassNames('panel-modal-header', headerClassName)}>
          <div className="panel-modal-top-row">
            <div className="panel-modal-top-leading">
              <div className="panel-modal-header-leading">
                {origin === 'settings' && onBack ? (
                  <button
                    type="button"
                    className="btn btn-icon panel-modal-back"
                    onClick={onBack}
                    aria-label={backLabel}
                    title={backLabel}
                  >
                    <ArrowLeft size={16} />
                  </button>
                ) : null}
                {headerLeading}
              </div>
              {badge ? (
                <div className={joinClassNames('panel-modal-badge', badgeClassName)}>
                  {badge}
                </div>
              ) : null}
            </div>
            <div className={joinClassNames('panel-modal-header-controls', headerControlsClassName)}>
              {headerActions ? (
                <div className={joinClassNames('panel-modal-toolbar', toolbarClassName)}>
                  {headerActions}
                </div>
              ) : null}
            </div>
            <div className="panel-modal-close-slot">
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
          <div className={joinClassNames('panel-modal-header-copy', headerCopyClassName)}>
            {typeof title === 'string'
              ? <h2 id={ariaLabelledby}>{title}</h2>
              : title}
            {description ? <p>{description}</p> : null}
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
