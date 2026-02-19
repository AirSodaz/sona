import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { XIcon } from './Icons';

interface ParameterSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    enableTimeline: boolean;
    setEnableTimeline: (value: boolean) => void;
    language: string;
    setLanguage: (value: string) => void;
    disabled?: boolean;
    lockWindow?: boolean;
    setLockWindow?: (value: boolean) => void;
    alwaysOnTop?: boolean;
    setAlwaysOnTop?: (value: boolean) => void;
}

/**
 * Modal for configuring transcription parameters (Subtitle Mode, Language).
 */
export function ParameterSettingsModal({
    isOpen,
    onClose,
    enableTimeline,
    setEnableTimeline,
    language,
    setLanguage,
    disabled = false,
    lockWindow,
    setLockWindow,
    alwaysOnTop,
    setAlwaysOnTop
}: ParameterSettingsModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const modalRef = useRef<HTMLDivElement>(null);
    const closeButtonRef = useRef<HTMLButtonElement>(null);

    // Focus management
    useEffect(() => {
        if (isOpen) {
            requestAnimationFrame(() => {
                closeButtonRef.current?.focus();
            });
        }
    }, [isOpen]);

    // Keyboard support (Escape to close)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (!isOpen) return;
            if (e.key === 'Escape') {
                e.preventDefault();
                onClose();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isOpen, onClose]);

    if (!isOpen) return null;

    return (
        <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
            <div
                ref={modalRef}
                className="dialog-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                aria-labelledby="param-settings-title"
                style={{
                    background: 'var(--color-bg-elevated)',
                    borderRadius: 'var(--radius-lg)',
                    boxShadow: 'var(--shadow-xl)',
                    width: '450px',
                    maxWidth: '90vw',
                    padding: 'var(--spacing-lg)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 'var(--spacing-md)',
                    border: '1px solid var(--color-border)',
                }}
            >
                {/* Header */}
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <h3
                        id="param-settings-title"
                        style={{
                            fontSize: '1.125rem',
                            fontWeight: 600,
                            color: 'var(--color-text-primary)',
                            margin: 0
                        }}
                    >
                        {t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
                    </h3>
                    <button
                        ref={closeButtonRef}
                        className="btn btn-icon"
                        onClick={onClose}
                        aria-label={t('common.close')}
                    >
                        <XIcon />
                    </button>
                </div>

                {/* Content */}
                <div className="options-container" style={{ padding: 0, display: 'flex', flexDirection: 'column', gap: 'var(--spacing-lg)' }}>
                    {/* Subtitle Mode */}
                    <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <div className="options-label">
                            <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('batch.timeline_mode')}</span>
                            <span className="options-hint" style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>{t('batch.timeline_hint')}</span>
                        </div>
                        <button
                            className={`toggle-switch ${disabled ? 'disabled' : ''}`}
                            onClick={() => !disabled && setEnableTimeline(!enableTimeline)}
                            role="switch"
                            aria-checked={enableTimeline}
                            aria-label={t('batch.timeline_mode')}
                            disabled={disabled}
                        >
                            <div className="toggle-switch-handle" />
                        </button>
                    </div>

                    {/* Lock Window (Click-through) */}
                    {typeof lockWindow !== 'undefined' && setLockWindow && (
                        <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                            <div className="options-label">
                                <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('live.lock_window', { defaultValue: 'Lock Window' })}</span>
                                <span className="options-hint" style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>{t('live.lock_window_hint', { defaultValue: 'Make window click-through' })}</span>
                            </div>
                            <button
                                className={`toggle-switch ${disabled ? 'disabled' : ''}`}
                                onClick={() => !disabled && setLockWindow(!lockWindow)}
                                role="switch"
                                aria-checked={lockWindow}
                                aria-label={t('live.lock_window', { defaultValue: 'Lock Window' })}
                                disabled={disabled}
                            >
                                <div className="toggle-switch-handle" />
                            </button>
                        </div>
                    )}

                    {/* Always on Top */}
                    {typeof alwaysOnTop !== 'undefined' && setAlwaysOnTop && (
                        <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                            <div className="options-label">
                                <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('live.always_on_top', { defaultValue: 'Always on Top' })}</span>
                                <span className="options-hint" style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>{t('live.always_on_top_hint', { defaultValue: 'Keep window above others' })}</span>
                            </div>
                            <button
                                className={`toggle-switch ${disabled ? 'disabled' : ''}`}
                                onClick={() => !disabled && setAlwaysOnTop(!alwaysOnTop)}
                                role="switch"
                                aria-checked={alwaysOnTop}
                                aria-label={t('live.always_on_top', { defaultValue: 'Always on Top' })}
                                disabled={disabled}
                            >
                                <div className="toggle-switch-handle" />
                            </button>
                        </div>
                    )}

                    {/* Language */}
                    <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <div className="options-label">
                            <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('batch.language')}</span>
                            <span className="options-hint" style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>{t('batch.language_hint')}</span>
                        </div>
                        <Dropdown
                            value={language}
                            onChange={(val) => !disabled && setLanguage(val)}
                            options={[
                                { value: 'auto', label: 'Auto' },
                                { value: 'zh', label: 'Chinese' },
                                { value: 'en', label: 'English' },
                                { value: 'ja', label: 'Japanese' },
                                { value: 'ko', label: 'Korean' },
                                { value: 'yue', label: 'Cantonese' }
                            ]}
                            style={{ width: '180px', opacity: disabled ? 0.6 : 1, pointerEvents: disabled ? 'none' : 'auto' }}
                        />
                    </div>
                </div>

                {/* Footer (optional close button, though X is top right) */}
                 {/* No footer needed as per design usually */}
            </div>
        </div>
    );
}
