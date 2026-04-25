import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, AlertTriangle, CheckCircle, Info, Loader2 } from 'lucide-react';
import { useDialogStore } from '../stores/dialogStore';
import { SparklesIcon } from './Icons';

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
 * Renders alert, confirm or prompt dialogs based on the dialog store state.
 * Handles focus management and keyboard interaction (Escape to close).
 *
 * @return The rendered dialog or null if closed.
 */
export function GlobalDialog(): React.JSX.Element | null {
    const { isOpen, options, close } = useDialogStore();
    const { t } = useTranslation();
    const confirmButtonRef = useRef<HTMLButtonElement>(null);
    const cancelButtonRef = useRef<HTMLButtonElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const modalRef = useRef<HTMLDivElement>(null);
    const [inputValue, setInputValue] = useState('');
    const [isAiLoading, setIsAiLoading] = useState(false);

    // Reset input value when dialog opens
    useEffect(() => {
        if (isOpen && options?.type === 'prompt') {
            setInputValue(options.defaultValue || '');
            setIsAiLoading(false);
        }
    }, [isOpen, options]);

    const handleAiClick = async () => {
        if (!options?.onAiAction) return;
        setIsAiLoading(true);
        try {
            const result = await options.onAiAction();
            setInputValue(result);
        } catch (error) {
            // Error handling is managed by the service
        } finally {
            setIsAiLoading(false);
        }
    };

    // Focus management
    useEffect(() => {
        if (isOpen) {
            // Wait for render
            requestAnimationFrame(() => {
                if (options?.type === 'prompt') {
                    inputRef.current?.focus();
                    inputRef.current?.select();
                } else if (options?.type === 'confirm') {
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
        function handleKeyDown(e: KeyboardEvent) {
            if (!isOpen) return;

            if (e.key === 'Escape') {
                e.preventDefault();
                close(options?.type === 'prompt' ? null : false);
                return;
            }

            if (e.key === 'Enter' && options?.type === 'prompt') {
                e.preventDefault();
                close(inputValue);
                return;
            }

            if (e.key === 'Tab') {
                if (!modalRef.current) return;

                const focusableElements = modalRef.current.querySelectorAll(
                    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
                );

                if (focusableElements.length === 0) return;

                const firstElement = focusableElements[0] as HTMLElement;
                const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                if (e.shiftKey && document.activeElement === firstElement) {
                    e.preventDefault();
                    lastElement.focus();
                } else if (!e.shiftKey && document.activeElement === lastElement) {
                    e.preventDefault();
                    firstElement.focus();
                }
            }
        }

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isOpen, close, options?.type, inputValue]);

    if (!isOpen || !options) return null;

    const {
        title,
        message,
        details,
        type = 'alert',
        variant = 'info',
        confirmLabel,
        cancelLabel,
        inputPlaceholder,
        onAiAction,
    } = options;

    const handleConfirm = () => {
        if (type === 'prompt') {
            close(inputValue);
        } else {
            close(true);
        }
    };

    const handleCancel = () => {
        if (type === 'prompt') {
            close(null);
        } else {
            close(false);
        }
    };

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
                                overflowWrap: 'anywhere',
                                wordBreak: 'break-word',
                                whiteSpace: 'pre-wrap',
                            }}
                        >
                            {message}
                        </p>
                        
                        {type === 'prompt' && (
                            <div style={{ marginTop: 'var(--spacing-md)', position: 'relative', display: 'flex', alignItems: 'center' }}>
                                <input
                                    ref={inputRef}
                                    type="text"
                                    className="input"
                                    value={inputValue}
                                    onChange={(e) => setInputValue(e.target.value)}
                                    placeholder={inputPlaceholder}
                                    style={{
                                        width: '100%',
                                        padding: 'var(--spacing-sm)',
                                        paddingRight: onAiAction ? '36px' : 'var(--spacing-sm)',
                                        borderRadius: 'var(--radius-md)',
                                        border: '1px solid var(--color-border)',
                                        background: 'var(--color-bg-subtle)',
                                        color: 'var(--color-text-primary)',
                                    }}
                                />
                                {onAiAction && (
                                    <button
                                        type="button"
                                        className="btn btn-icon btn-sm"
                                        onClick={handleAiClick}
                                        disabled={isAiLoading}
                                        title={t('common.ai_rename', { defaultValue: 'AI Auto-rename' })}
                                        style={{
                                            position: 'absolute',
                                            right: '4px',
                                            padding: '4px',
                                            color: 'var(--color-info)',
                                            background: 'transparent',
                                            border: 'none',
                                            display: 'flex',
                                            alignItems: 'center',
                                            justifyContent: 'center',
                                            cursor: isAiLoading ? 'default' : 'pointer',
                                        }}
                                    >
                                        {isAiLoading ? (
                                            <Loader2 className="animate-spin" width={16} height={16} />
                                        ) : (
                                            <SparklesIcon width={16} height={16} />
                                        )}
                                    </button>
                                )}
                            </div>
                        )}

                        {details && (
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
                                        overflowWrap: 'anywhere',
                                        wordBreak: 'break-word',
                                        whiteSpace: 'pre-wrap',
                                        margin: 0,
                                    }}
                                >
                                    {details}
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
                    {(type === 'confirm' || type === 'prompt') && (
                        <button
                            ref={cancelButtonRef}
                            className="btn btn-secondary"
                            onClick={handleCancel}
                        >
                            {cancelLabel || t('common.cancel', { defaultValue: 'Cancel' })}
                        </button>
                    )}
                    <button
                        ref={confirmButtonRef}
                        className={`btn ${variant === 'error' ? 'btn-danger' : 'btn-primary'}`}
                        onClick={handleConfirm}
                    >
                        {confirmLabel || (type === 'confirm' || type === 'prompt' ? t('common.confirm', { defaultValue: 'Confirm' }) : t('common.ok', { defaultValue: 'OK' }))}
                    </button>
                </div>
            </div>
        </div>
    );
}
