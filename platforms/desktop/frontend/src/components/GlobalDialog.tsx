import React, { useEffect, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { AlertCircle, AlertTriangle, CheckCircle, Info, Loader2 } from 'lucide-react';
import { type DialogVariant, useDialogStore } from '../stores/dialogStore';
import { SparklesIcon } from './Icons';
import { Modal } from './Modal';
import { logger } from '../utils/logger';

/**
 * Renders the appropriate icon based on dialog variant.
 */
function DialogIcon({ variant }: { variant: DialogVariant }): React.JSX.Element {
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
function getDialogTitle(t: TFunction, variant: DialogVariant, title?: string): string {
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
    const [inputValue, setInputValue] = useState('');
    const [isAiLoading, setIsAiLoading] = useState(false);

    // Reset input value when dialog opens
    useEffect(() => {
        if (!isOpen || options?.type !== 'prompt') {
            return;
        }

        queueMicrotask(() => {
            setInputValue(options.defaultValue || '');
            setIsAiLoading(false);
        });
    }, [isOpen, options]);

    const handleAiClick = async () => {
        if (!options?.onAiAction) return;
        setIsAiLoading(true);
        try {
            const result = await options.onAiAction();
            setInputValue(result);
        } catch {
            // Error handling is managed by the service — log for diagnostics.
            logger.debug('[GlobalDialog] AI action failed (service handles error)');
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

    // Remove custom Keyboard support as Modal handles Escape/Tab
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
        <Modal
            isOpen={isOpen}
            onClose={handleCancel}
            closeOnOverlayClick={type !== 'prompt'}
            size={options.type === 'prompt' ? 'md' : 'sm'}
            overlayStyle={{ zIndex: 2200 }}
            shellStyle={{ overflow: 'visible' }}
            bodyStyle={{ overflow: 'visible' }}
            role="alertdialog"
            title={
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)' }}>
                    <DialogIcon variant={variant} />
                    <span>{getDialogTitle(t, variant, title)}</span>
                </div>
            }
            footer={
                <>
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
                </>
            }
        >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
                <p
                    id="dialog-desc"
                    style={{
                        fontSize: '0.9rem',
                        color: 'var(--color-text-secondary)',
                        lineHeight: 1.5,
                        overflowWrap: 'anywhere',
                        wordBreak: 'break-word',
                        whiteSpace: 'pre-wrap',
                        margin: 0,
                    }}
                >
                    {message}
                </p>

                {type === 'prompt' && (
                    <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                        <input
                            ref={inputRef}
                            type="text"
                            className="input"
                            value={inputValue}
                            onChange={(e) => setInputValue(e.target.value)}
                            placeholder={inputPlaceholder}
                            onKeyDown={(e) => {
                                if (e.key === 'Enter') {
                                    e.preventDefault();
                                    handleConfirm();
                                }
                            }}
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
                                data-tooltip={t('common.ai_rename', { defaultValue: 'AI Auto-rename' })}
                                data-tooltip-pos="top"
                                aria-label={t('common.ai_rename', { defaultValue: 'AI Auto-rename' })}
                                style={{
                                    position: 'absolute',
                                    right: '4px',
                                    top: 0,
                                    bottom: 0,
                                    marginTop: 'auto',
                                    marginBottom: 'auto',
                                    width: '28px',
                                    height: '28px',
                                    padding: 0,
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
        </Modal>
    );
}
