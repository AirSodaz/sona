import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle } from 'lucide-react';
import { useErrorDialogStore } from '../stores/errorDialogStore';

/**
 * Dedicated modal dialog for user-visible errors.
 */
export function ErrorDialog(): React.JSX.Element | null {
  const { t } = useTranslation();
  const { isOpen, options, close } = useErrorDialogStore();
  const primaryButtonRef = useRef<HTMLButtonElement>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    requestAnimationFrame(() => {
      if (options?.hasPrimaryAction) {
        cancelButtonRef.current?.focus();
      } else {
        primaryButtonRef.current?.focus();
      }
    });
  }, [isOpen, options]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (!isOpen) {
        return;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        close('dismiss');
        return;
      }

      if (event.key !== 'Tab' || !modalRef.current) {
        return;
      }

      const focusableElements = modalRef.current.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );

      if (focusableElements.length === 0) {
        return;
      }

      const firstElement = focusableElements[0] as HTMLElement;
      const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, close]);

  if (!isOpen || !options) {
    return null;
  }

  return (
    <div className="settings-overlay" style={{ zIndex: 2001 }}>
      <div
        ref={modalRef}
        className="dialog-modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="error-dialog-title"
        aria-describedby="error-dialog-desc"
        style={{
          background: 'var(--color-bg-elevated)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: 'var(--shadow-xl)',
          width: '400px',
          maxWidth: '90vw',
          padding: 'var(--spacing-lg)',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-md)',
          border: '1px solid var(--color-border)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'start', gap: 'var(--spacing-md)' }}>
          <div style={{ flexShrink: 0, paddingTop: 4 }}>
            <AlertCircle className="w-6 h-6 text-red-500" style={{ color: 'var(--color-error)' }} />
          </div>
          <div style={{ flex: 1 }}>
            <h3
              id="error-dialog-title"
              style={{
                fontSize: '1.125rem',
                fontWeight: 600,
                color: 'var(--color-text-primary)',
                marginBottom: 'var(--spacing-xs)',
              }}
            >
              {options.title}
            </h3>
            <p
              id="error-dialog-desc"
              style={{
                fontSize: '0.9rem',
                color: 'var(--color-text-secondary)',
                lineHeight: 1.5,
                overflowWrap: 'anywhere',
                wordBreak: 'break-word',
                whiteSpace: 'pre-wrap',
              }}
            >
              {options.message}
            </p>
            {options.details && (
              <div
                style={{
                  marginTop: 'var(--spacing-sm)',
                  paddingTop: 'var(--spacing-sm)',
                  borderTop: '1px solid var(--color-border)',
                }}
              >
                <div
                  style={{
                    fontSize: '0.75rem',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    marginBottom: '4px',
                  }}
                >
                  {t('common.details', { defaultValue: 'Details' })}
                </div>
                <p
                  style={{
                    fontSize: '0.82rem',
                    color: 'var(--color-text-secondary)',
                    lineHeight: 1.5,
                    margin: 0,
                    overflowWrap: 'anywhere',
                    wordBreak: 'break-word',
                    whiteSpace: 'pre-wrap',
                  }}
                >
                  {options.details}
                </p>
              </div>
            )}
          </div>
        </div>

        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            gap: 'var(--spacing-sm)',
            marginTop: 'var(--spacing-sm)',
          }}
        >
          {options.hasPrimaryAction && (
            <button
              ref={cancelButtonRef}
              className="btn btn-secondary"
              onClick={() => close('dismiss')}
            >
              {options.cancelLabel ?? t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
          )}
          <button
            ref={primaryButtonRef}
            className={options.hasPrimaryAction ? 'btn btn-danger' : 'btn btn-primary'}
            onClick={() => close(options.hasPrimaryAction ? 'primary' : 'dismiss')}
          >
            {options.primaryLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
