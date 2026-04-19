import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { XIcon } from './Icons';
import { Dropdown } from './Dropdown';
import { Switch } from './Switch';
import { isFeatureLlmConfigComplete } from '../services/llmConfig';

interface PolishSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

/**
 * Modal for configuring advanced polish settings.
 */
export function PolishSettingsModal({ isOpen, onClose }: PolishSettingsModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const config = useConfigStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);

    const autoPolish = config.autoPolish ?? false;
    const autoPolishFrequency = config.autoPolishFrequency ?? 5;
    const isLlmConfigured = isFeatureLlmConfigComplete(config, 'polish');

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

    const scenarioOptions = [
        { value: 'customer_service', label: t('polish.scenarios.customer_service') },
        { value: 'meeting', label: t('polish.scenarios.meeting') },
        { value: 'interview', label: t('polish.scenarios.interview') },
        { value: 'lecture', label: t('polish.scenarios.lecture') },
        { value: 'podcast', label: t('polish.scenarios.podcast') },
        { value: 'custom', label: t('polish.scenarios.custom') },
    ];

    return (
        <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
            <div
                className="dialog-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                aria-labelledby="polish-settings-modal-title"
                style={{
                    background: 'var(--color-bg-elevated)',
                    borderRadius: 'var(--radius-lg)',
                    boxShadow: 'var(--shadow-xl)',
                    width: '500px',
                    maxWidth: '95vw',
                    padding: 'var(--spacing-lg)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 'var(--spacing-md)',
                    border: '1px solid var(--color-border)',
                }}
            >
                {/* Header */}
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <h3 id="polish-settings-modal-title" style={{ fontSize: '1.125rem', fontWeight: 600, color: 'var(--color-text-primary)', margin: 0 }}>
                        {t('polish.advanced_settings')}
                    </h3>
                    <button
                        className="btn btn-icon"
                        onClick={onClose}
                        aria-label={t('common.close')}
                        data-tooltip={t('common.close')}
                        data-tooltip-pos="bottom-left"
                    >
                        <XIcon />
                    </button>
                </div>

                {/* Content */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-lg)' }}>
                    {/* Auto Polish */}
                    <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between' }}>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)', flex: 1 }}>
                            <span style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>{t('batch.auto_polish', { defaultValue: 'Auto-Polish' })}</span>
                            <span style={{ fontSize: '0.75rem', color: 'var(--color-text-secondary)', lineHeight: 1.4 }}>
                                {isLlmConfigured
                                    ? t('batch.auto_polish_hint', { defaultValue: 'Automatically polish text with LLM' })
                                    : t('polish.error_config_missing', { defaultValue: 'Please configure LLM service first' })}
                            </span>
                        </div>
                        <Switch
                            checked={autoPolish}
                            onChange={(val) => isLlmConfigured && setConfig({ autoPolish: val })}
                            disabled={!isLlmConfigured}
                        />
                    </div>

                    {/* Auto Polish Frequency */}
                    {autoPolish && (
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                            <label htmlFor="auto-polish-frequency" style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                                {t('batch.auto_polish_frequency', { defaultValue: 'Auto-Polish Frequency' })}
                            </label>
                            <input
                                id="auto-polish-frequency"
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
                                style={{
                                    width: '80px',
                                    padding: '6px 10px',
                                    borderRadius: '4px',
                                    border: '1px solid var(--color-border)',
                                    backgroundColor: 'var(--color-bg-input)',
                                    color: 'var(--color-text-primary)',
                                    fontSize: '0.875rem',
                                    outline: 'none'
                                }}
                            />
                        </div>
                    )}

                    <div style={{ height: '1px', background: 'var(--color-border)', opacity: 0.5, margin: '4px 0' }} />

                    {/* Keywords */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                        <label htmlFor="polish-keywords" style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                            {t('polish.keywords')}
                        </label>
                        <input
                            id="polish-keywords"
                            type="text"
                            value={config.polishKeywords || ''}
                            onChange={(e) => setConfig({ polishKeywords: e.target.value })}
                            style={{
                                padding: '8px 12px',
                                borderRadius: '4px',
                                border: '1px solid var(--color-border)',
                                background: 'var(--color-bg-input)',
                                color: 'var(--color-text-primary)',
                                outline: 'none'
                            }}
                            placeholder={t('polish.keywords_placeholder')}
                        />
                    </div>

                    {/* Scenario */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                        <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                            {t('polish.scenario_label')}
                        </label>
                        <Dropdown
                            value={config.polishScenario || 'custom'}
                            onChange={(val) => setConfig({ polishScenario: val })}
                            options={scenarioOptions}
                            style={{ width: '100%' }}
                        />
                    </div>

                    {/* Custom Context */}
                    {(config.polishScenario === 'custom' || !config.polishScenario) && (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                            <label htmlFor="polish-custom-context" style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                                {t('polish.custom_context')}
                            </label>
                            <textarea
                                id="polish-custom-context"
                                value={config.polishContext || ''}
                                onChange={(e) => setConfig({ polishContext: e.target.value })}
                                style={{
                                    padding: '8px 12px',
                                    borderRadius: '4px',
                                    border: '1px solid var(--color-border)',
                                    background: 'var(--color-bg-input)',
                                    color: 'var(--color-text-primary)',
                                    outline: 'none',
                                    minHeight: '80px',
                                    resize: 'vertical',
                                    fontFamily: 'inherit'
                                }}
                                placeholder={t('polish.context_placeholder')}
                            />
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 'var(--spacing-sm)', marginTop: 'var(--spacing-md)' }}>
                    <button className="btn btn-primary" onClick={onClose}>
                        {t('common.close')}
                    </button>
                </div>
            </div>
        </div>
    );
}
