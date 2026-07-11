import React, { useEffect, useId, useRef } from 'react';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getFocusableElements, isTopMostModal } from '../utils/focusUtils';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { ModalPortal } from './ModalPortal';

function joinClassNames(...parts: Array<string | undefined | false | null>): string {
  return parts.filter(Boolean).join(' ');
}

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
  children: React.ReactNode;
  footer?: React.ReactNode;
  closeOnOverlayClick?: boolean;
  closeOnEsc?: boolean;
  hideCloseButton?: boolean;
  className?: string;
  initialFocusRef?: React.RefObject<HTMLElement | null>;
  autoFocus?: boolean;
  overlayClassName?: string;
  overlayStyle?: React.CSSProperties;
  shellStyle?: React.CSSProperties;
  bodyStyle?: React.CSSProperties;
  role?: 'dialog' | 'alertdialog';
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
  hideCloseButton = false,
  className,
  initialFocusRef,
  autoFocus = true,
  overlayClassName,
  overlayStyle,
  shellStyle,
  bodyStyle,
  role = 'dialog',
}: ModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const modalRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);
  const bodyContentRef = useRef<HTMLDivElement>(null);
  const footerRef = useRef<HTMLDivElement>(null);

  const previousFocusRef = useRef<HTMLElement | null>(null);
  const generatedTitleId = useId();
  const titleId = title ? `${generatedTitleId}-title` : undefined;

  const hasTitle = Boolean(title);
  const hasFooter = Boolean(footer);

  // Smooth resize logic
  useEffect(() => {
    if (!isOpen) {
      if (modalRef.current) {
        modalRef.current.style.height = '';
      }
      return;
    }

    let cachedPaddingTotal: number | null = null;
    let isInitialMeasurement = true;
    let rafId: number | null = null;

    const updateHeight = () => {
      if (!modalRef.current) return;

      const headerH = headerRef.current?.offsetHeight || 0;
      const footerH = footerRef.current?.offsetHeight || 0;
      const bodyContentH = bodyContentRef.current?.offsetHeight || 0;

      if (cachedPaddingTotal === null && bodyContentRef.current?.parentElement) {
        const style = window.getComputedStyle(bodyContentRef.current.parentElement);
        cachedPaddingTotal = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom);
      }

      const paddingTotal = cachedPaddingTotal || 0;
      // Add 2px for the top and bottom borders of .shared-modal-shell
      const totalHeight = headerH + footerH + bodyContentH + paddingTotal + 2;

      if (isInitialMeasurement) {
        modalRef.current.style.transition = 'none';
        modalRef.current.style.height = `${totalHeight}px`;
        // Force reflow
        void modalRef.current.offsetHeight;
        modalRef.current.style.transition = '';
        isInitialMeasurement = false;
      } else {
        if (rafId !== null) {
          cancelAnimationFrame(rafId);
        }
        rafId = requestAnimationFrame(() => {
          if (modalRef.current) {
            modalRef.current.style.height = `${totalHeight}px`;
          }
        });
      }
    };

    const observer = new ResizeObserver(updateHeight);

    if (headerRef.current) observer.observe(headerRef.current);
    if (bodyContentRef.current) observer.observe(bodyContentRef.current);
    if (footerRef.current) observer.observe(footerRef.current);

    // Initial measurement
    const timeoutId = setTimeout(updateHeight, 0);

    return () => {
      observer.disconnect();
      clearTimeout(timeoutId);
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
      }
    };
  }, [isOpen, hasTitle, hasFooter]);

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
          const focusable = getFocusableElements(modalRef.current);
          if (focusable.length > 0) {
            focusable[0].focus();
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
  useEscapeKey((event) => {
    event.preventDefault();
    event.stopImmediatePropagation();
    onClose();
  }, {
    enabled: isOpen && closeOnEsc,
    checkTopMost: true,
    containerRef: modalRef
  });

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;

      const isTopMost = isTopMostModal(modalRef.current);

      // 2. Tab Key Focus Trap
      if (event.key === 'Tab') {
        if (!isTopMost || !modalRef.current) return;

        const focusable = getFocusableElements(modalRef.current);
        if (focusable.length === 0) return;

        const first = focusable[0];
        const last = focusable[focusable.length - 1];

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
  }, [isOpen]);

  if (!isOpen) return null;

  const handleOverlayClick = () => {
    if (closeOnOverlayClick) {
      onClose();
    }
  };

  const combinedShellStyle: React.CSSProperties = {
    ...shellStyle,
  };

  return (
    <ModalPortal>
      <div
        className={joinClassNames('shared-modal-overlay', overlayClassName)}
        style={overlayStyle}
        data-modal-layer="shared-modal"
        onClick={handleOverlayClick}
      >
        <div
          ref={modalRef}
          className={joinClassNames(`shared-modal-shell shared-modal-${size}`, className)}
          style={combinedShellStyle}
          onClick={(e) => e.stopPropagation()}
          role={role}
          aria-modal="true"
          aria-labelledby={titleId}
          tabIndex={-1}
        >
          {/* Header */}
          <div className="shared-modal-header" ref={headerRef}>
            {title && (
              typeof title === 'string' ? (
                <h3 className="shared-modal-title" id={titleId}>{title}</h3>
              ) : (
                <div className="shared-modal-title" id={titleId}>{title}</div>
              )
            )}
            {!hideCloseButton && (
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
            )}
          </div>

          {/* Body */}
          <div className="shared-modal-body" style={bodyStyle}>
            <div ref={bodyContentRef}>
              {children}
            </div>
          </div>

          {/* Footer */}
          {footer && (
            <div className="shared-modal-footer" ref={footerRef}>
              {footer}
            </div>
          )}
        </div>
      </div>
    </ModalPortal>
  );
}
