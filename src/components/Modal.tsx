import React, { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ModalPortal } from './ModalPortal';

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
  children: React.ReactNode;
  footer?: React.ReactNode;
  closeOnOverlayClick?: boolean;
  closeOnEsc?: boolean;
  initialFocusRef?: React.RefObject<HTMLElement | null>;
  autoFocus?: boolean;
}

export function Modal({
  isOpen,
  onClose,
  title,
  size = 'md',
  children,
  footer,
  closeOnOverlayClick = true,
  closeOnEsc = true,
  initialFocusRef,
  autoFocus = true,
}: ModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const modalRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  // Focus trap and focus restoration
  useEffect(() => {
    if (isOpen) {
      // Save current active element to restore focus later
      previousFocusRef.current = document.activeElement as HTMLElement;

      let focusTimer: ReturnType<typeof setTimeout> | null = null;

      if (autoFocus) {
        focusTimer = setTimeout(() => {
          if (initialFocusRef?.current) {
            initialFocusRef.current.focus();
            return;
          }

          if (!modalRef.current) return;
          const focusable = modalRef.current.querySelectorAll(
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
          );
          if (focusable.length > 0) {
            (focusable[0] as HTMLElement).focus();
          }
        }, 50);
      }

      return () => {
        if (focusTimer) clearTimeout(focusTimer);
        if (previousFocusRef.current) {
          previousFocusRef.current.focus();
          previousFocusRef.current = null;
        }
      };
    }
  }, [isOpen, autoFocus, initialFocusRef]);

  // Keyboard events (Esc key and Focus Trap Tab key)
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;

      const overlays = document.querySelectorAll('.shared-modal-overlay, .panel-modal-overlay, .settings-overlay, [data-focus-trap-overlay]');
      const topOverlay = overlays[overlays.length - 1];
      const isTopMost = topOverlay && topOverlay.contains(modalRef.current);

      // 1. Esc Key
      if (event.key === 'Escape' && closeOnEsc) {
        if (isTopMost) {
          event.preventDefault();
          event.stopImmediatePropagation();
          onClose();
        }
        return;
      }

      // 2. Tab Key Focus Trap
      if (event.key === 'Tab') {
        if (!isTopMost || !modalRef.current) return;

        const focusable = modalRef.current.querySelectorAll(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;

        const first = focusable[0] as HTMLElement;
        const last = focusable[focusable.length - 1] as HTMLElement;

        const isFocusInside = modalRef.current.contains(document.activeElement);

        if (!isFocusInside) {
          event.preventDefault();
          first.focus();
        } else if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose, closeOnEsc]);

  if (!isOpen) return null;

  const handleOverlayClick = () => {
    if (closeOnOverlayClick) {
      onClose();
    }
  };

  return (
    <ModalPortal>
      <div 
        className="shared-modal-overlay" 
        onClick={handleOverlayClick}
      >
        <div
          ref={modalRef}
          className={`shared-modal-shell shared-modal-${size}`}
          onClick={(e) => e.stopPropagation()}
          role="dialog"
          aria-modal="true"
          aria-labelledby={title ? 'shared-modal-title-id' : undefined}
          tabIndex={-1}
        >
          {/* Header */}
          <div className="shared-modal-header">
            {title && (
              typeof title === 'string' ? (
                <h3 className="shared-modal-title" id="shared-modal-title-id">{title}</h3>
              ) : (
                <div className="shared-modal-title" id="shared-modal-title-id">{title}</div>
              )
            )}
            <button
              type="button"
              className="btn btn-icon shared-modal-close-btn"
              onClick={onClose}
              aria-label={t('common.close', { defaultValue: 'Close' })}
              data-tooltip={t('common.close', { defaultValue: 'Close' })}
              data-tooltip-pos="bottom-left"
            >
              <X size={18} />
            </button>
          </div>

          {/* Body */}
          <div className="shared-modal-body">
            {children}
          </div>

          {/* Footer */}
          {footer && (
            <div className="shared-modal-footer">
              {footer}
            </div>
          )}
        </div>
      </div>
    </ModalPortal>
  );
}
