import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { Switch } from './Switch';
import { XIcon } from './Icons';
import { useTranscriptStore } from '../stores/transcriptStore';

interface ParameterSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    disabled?: boolean;
}

/**
 * Modal for configuring transcription parameters (Subtitle Mode, Language, Auto-Polish).
 */
export function ParameterSettingsModal({
    isOpen,
    onClose,
    disabled = false
}: ParameterSettingsModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const modalRef = useRef<HTMLDivElement>(null);
    const closeButtonRef = useRef<HTMLButtonElement>(null);

    // Get config and setters from store
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);

    // Derived values
    const enableTimeline = config.enableTimeline ?? false;
    const language = config.language;
    const autoPolish = config.autoPolish ?? false;
    const autoPolishFrequency = config.autoPolishFrequency ?? 5;
    const llm = config.llm;
    const isLlmConfigured = Boolean(llm?.apiKey && llm.baseUrl && llm.model && llm.provider);

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

    const dropdownStyle = {
        width: '180px',
        opacity: disabled ? 0.6 : 1,
        pointerEvents: disabled ? 'none' : 'auto'
    } as React.CSSProperties;

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
                            <span className="options-hint">{t('batch.timeline_hint')}</span>
                        </div>
                        <Switch
                            checked={enableTimeline}
                            onChange={(val) => !disabled && setConfig({ enableTimeline: val })}
                            disabled={disabled}
                        />
                    </div>

                    {/* Language */}
                    <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <div className="options-label">
                            <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('batch.language')}</span>
                            <span className="options-hint">{t('batch.language_hint')}</span>
                        </div>
                        <Dropdown
                            value={language}
                            onChange={(val) => !disabled && setConfig({ language: val })}
                            options={[
                                { value: 'auto', label: 'Auto' },
                                { value: 'zh', label: 'Chinese' },
                                { value: 'en', label: 'English' },
                                { value: 'ja', label: 'Japanese' },
                                { value: 'ko', label: 'Korean' },
                                { value: 'yue', label: 'Cantonese' }
                            ]}
                            style={dropdownStyle}
                        />
                    </div>

                    {/* Auto Polish */}
                    <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <div className="options-label">
                            <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('batch.auto_polish', { defaultValue: 'Auto-Polish' })}</span>
                            <span className="options-hint">
                                {isLlmConfigured
                                    ? t('batch.auto_polish_hint', { defaultValue: 'Automatically polish text with LLM' })
                                    : t('polish.error_config_missing', { defaultValue: 'Please configure LLM service first' })}
                            </span>
                        </div>
                        <Switch
                            checked={autoPolish}
                            onChange={(val) => !disabled && isLlmConfigured && setConfig({ autoPolish: val })}
                            disabled={disabled || !isLlmConfigured}
                        />
                    </div>

                    {/* Auto Polish Frequency */}
                    {autoPolish && (
                        <div className="options-row" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                            <div className="options-label">
                                <span style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{t('batch.auto_polish_frequency', { defaultValue: 'Auto-Polish Frequency' })}</span>
                            </div>
                            <input
                                type="number"
                                min={1}
                                max={100}
                                value={autoPolishFrequency}
                                onChange={(e) => {
                                    const val = parseInt(e.target.value, 10);
                                    if (!isNaN(val) && val > 0) {
                                        setConfig({ autoPolishFrequency: val });
                                    }
                                }}
                                disabled={disabled}
                                style={{
                                    width: '100px',
                                    padding: '8px',
                                    borderRadius: '4px',
                                    border: '1px solid var(--color-border)',
                                    backgroundColor: 'var(--color-bg-input)',
                                    color: 'var(--color-text)',
                                    textAlign: 'center'
                                }}
                            />
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
