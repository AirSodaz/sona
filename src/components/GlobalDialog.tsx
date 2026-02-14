import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, AlertTriangle, CheckCircle, Info } from 'lucide-react';
import { useDialogStore } from '../stores/dialogStore';

/**
 * Renders the appropriate icon based on dialog variant.
 */
function DialogIcon({ variant }: { variant: string }): React.JSX.Element {
    switch (variant) {
        case 'error':
            return <AlertCircle className="w-6 h-6 text-red-500" style={{ color: 'var(--color-error)' }} />;
        case 'warning':
            return <AlertTriangle className="w-6 h-6 text-amber-500" style={{ color: 'var(--color-warning)' }} />;
        case 'success':
            return <CheckCircle className="w-6 h-6 text-green-500" style={{ color: 'var(--color-success)' }} />;
        case 'info':
        default:
            return <Info className="w-6 h-6 text-blue-500" style={{ color: 'var(--color-info)' }} />;
    }
}

/**
 * Gets the dialog title based on variant if not provided.
 * Note: 't' is passed from the component to avoid hook rules in helper.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getDialogTitle(t: any, variant: string, title?: string): string {
    if (title) return title;
    switch (variant) {
        case 'error': return t('common.error', { defaultValue: 'Error' });
        case 'warning': return t('common.warning', { defaultValue: 'Warning' });
        case 'success': return t('common.success', { defaultValue: 'Success' });
        default: return t('common.info', { defaultValue: 'Info' });
    }
}

/**
 * Global modal dialog component.
 * Renders alert or confirm dialogs based on the dialog store state.
 * Handles focus management and keyboard interaction (Escape to close).
 *
 * @return The rendered dialog or null if closed.
 */
export function GlobalDialog(): React.JSX.Element | null {
    const { isOpen, options, close } = useDialogStore();
    const { t } = useTranslation();
    const confirmButtonRef = useRef<HTMLButtonElement>(null);
    const cancelButtonRef = useRef<HTMLButtonElement>(null);
    const modalRef = useRef<HTMLDivElement>(null);

    // Focus management
    useEffect(() => {
        if (isOpen) {
            // Wait for render
            requestAnimationFrame(() => {
                if (options?.type === 'confirm') {
                    // Focus cancel by default for safety in confirmations
                    cancelButtonRef.current?.focus();
                } else {
                    confirmButtonRef.current?.focus();
                }
            });
        }
    }, [isOpen, options]);

    // Keyboard support
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (!isOpen) return;

            if (e.key === 'Escape') {
                e.preventDefault();
                close(false);
            }

            if (e.key === 'Tab') {
                if (!modalRef.current) return;

                const focusableElements = modalRef.current.querySelectorAll(
                    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
                );

                if (focusableElements.length === 0) return;

                const firstElement = focusableElements[0] as HTMLElement;
                const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                if (e.shiftKey) {
                    if (document.activeElement === firstElement) {
                        e.preventDefault();
                        lastElement.focus();
                    }
                } else {
                    if (document.activeElement === lastElement) {
                        e.preventDefault();
                        firstElement.focus();
                    }
                }
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isOpen, close]);

    if (!isOpen || !options) return null;

    const {
        title,
        message,
        type = 'alert',
        variant = 'info',
        confirmLabel,
        cancelLabel,
    } = options;

    return (
        <div className="settings-overlay" style={{ zIndex: 2000 }}>
            <div
                ref={modalRef}
                className="dialog-modal"
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="dialog-title"
                aria-describedby="dialog-desc"
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
                        <DialogIcon variant={variant} />
                    </div>
                    <div style={{ flex: 1 }}>
                        <h3
                            id="dialog-title"
                            style={{
                                fontSize: '1.125rem',
                                fontWeight: 600,
                                color: 'var(--color-text-primary)',
                                marginBottom: 'var(--spacing-xs)',
                            }}
                        >
                            {getDialogTitle(t, variant, title)}
                        </h3>
                        <p
                            id="dialog-desc"
                            style={{
                                fontSize: '0.9rem',
                                color: 'var(--color-text-secondary)',
                                lineHeight: 1.5,
                            }}
                        >
                            {message}
                        </p>
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
                    {type === 'confirm' && (
                        <button
                            ref={cancelButtonRef}
                            className="btn btn-secondary"
                            onClick={() => close(false)}
                        >
                            {cancelLabel || t('common.cancel', { defaultValue: 'Cancel' })}
                        </button>
                    )}
                    <button
                        ref={confirmButtonRef}
                        className={`btn ${variant === 'error' ? 'btn-danger' : 'btn-primary'}`}
                        onClick={() => close(true)}
                    >
                        {confirmLabel || (type === 'confirm' ? t('common.confirm', { defaultValue: 'Confirm' }) : t('common.ok', { defaultValue: 'OK' }))}
                    </button>
                </div>
            </div>
        </div>
    );
}
