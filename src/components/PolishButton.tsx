import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { polishService } from '../services/polishService';
import { retranscribeService } from '../services/retranscribeService';
import { SparklesIcon, ChevronDownIcon, ChevronRightIcon, ProcessingIcon, RestoreIcon, RedoIcon, FileTextIcon } from './Icons';
import { TranscriptSegment } from '../types/transcript';

/** Props for PolishButton. */
interface PolishButtonProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Dropdown button component for polishing transcript segments (fixing errors).
 *
 * @param props - Component props.
 * @return The polish button component.
 */
export function PolishButton({ className = '' }: PolishButtonProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const { alert } = useDialogStore();
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const [showAdvanced, setShowAdvanced] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    // Undo/Redo state
    const [undoSegments, setUndoSegments] = useState<TranscriptSegment[] | null>(null);
    const [redoSegments, setRedoSegments] = useState<TranscriptSegment[] | null>(null);

    const segmentsLength = useTranscriptStore((state) => state.segments.length);

    // LLM state
    const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
    const llmState = useTranscriptStore((state) => state.llmStates[sourceHistoryId || 'current']) || { isPolishing: false, polishProgress: 0, isRetranscribing: false, retranscribeProgress: 0 };
    const { isPolishing, polishProgress, isRetranscribing, retranscribeProgress } = llmState;
    const updateLlmState = useTranscriptStore((state) => state.updateLlmState);

    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const setSegments = useTranscriptStore((state) => state.setSegments);
    const segments = useTranscriptStore((state) => state.segments);

    // Close dropdown when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        if (isOpen && dropdownRef.current) {
            const rect = dropdownRef.current.getBoundingClientRect();
            // Estimate dropdown height
            const estimatedHeight = 100;
            const spaceBelow = window.innerHeight - rect.bottom;

            if (spaceBelow < estimatedHeight && rect.top > estimatedHeight) {
                setPosition('top');
            } else {
                setPosition('bottom');
            }
        }

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isOpen]);

    // Focus management
    useEffect(() => {
        if (isOpen && menuRef.current) {
            const firstButton = menuRef.current.querySelector('button');
            if (firstButton) {
                requestAnimationFrame(() => firstButton.focus());
            }
        }
    }, [isOpen]);

    const handleBlur = (e: React.FocusEvent) => {
        if (!dropdownRef.current?.contains(e.relatedTarget as Node)) {
            setIsOpen(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isOpen) return;
        if (e.key === 'Escape') {
            e.preventDefault();
            setIsOpen(false);
            triggerRef.current?.focus();
            return;
        }

        if (menuRef.current) {
            const buttons = Array.from(menuRef.current.querySelectorAll('button'));
            const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);

            switch (e.key) {
                case 'ArrowDown':
                    e.preventDefault();
                    buttons[(currentIndex + 1) % buttons.length].focus();
                    break;
                case 'ArrowUp':
                    e.preventDefault();
                    buttons[(currentIndex - 1 + buttons.length) % buttons.length].focus();
                    break;
                case 'Home':
                    e.preventDefault();
                    buttons[0].focus();
                    break;
                case 'End':
                    e.preventDefault();
                    buttons[buttons.length - 1].focus();
                    break;
            }
        }
    };

    const handleRetranscribe = async () => {
        if (isRetranscribing) return;

        if (!config.offlineModelPath) {
            await alert(t('batch.no_model_error'), { variant: 'error' });
            return;
        }

        setIsOpen(false);
        triggerRef.current?.focus();

        updateLlmState({ isRetranscribing: true, retranscribeProgress: 0 });

        try {
            await retranscribeService.retranscribeCurrentRecord((progress) => {
                updateLlmState({ retranscribeProgress: progress });
            });

            // Clear old polish undo/redo states only after successful re-transcription
            setUndoSegments(null);
            setRedoSegments(null);
        } catch (error: any) {
            await alert(error.message || 'Unknown error', { variant: 'error' });
        } finally {
            updateLlmState({ isRetranscribing: false, retranscribeProgress: 0 });
        }
    };

    const handleStartPolish = async () => {
        if (isPolishing) return;

        const llm = config.llm;
        if (!llm?.apiKey || !llm.baseUrl || !llm.model || !llm.provider) {
            await alert(t('polish.error_config_missing'), { variant: 'error' });
            return;
        }

        // Save current segments for undo
        setUndoSegments(JSON.parse(JSON.stringify(segments)));
        setRedoSegments(null);

        setIsOpen(false);
        triggerRef.current?.focus();

        try {
            await polishService.polishTranscript();
        } catch (error: any) {
            await alert(t('polish.error_failed') + (error.message || 'Unknown error'), { variant: 'error' });
        }
    };

    const handleUndoPolish = () => {
        if (undoSegments) {
            // Save current state for redo
            setRedoSegments(JSON.parse(JSON.stringify(segments)));

            setSegments(undoSegments);
            setUndoSegments(null);
            setIsOpen(false);
            triggerRef.current?.focus();
        }
    };

    const handleRedoPolish = () => {
        if (redoSegments) {
            // Save current state (original) back to undo
            setUndoSegments(JSON.parse(JSON.stringify(segments)));

            setSegments(redoSegments);
            setRedoSegments(null);
            setIsOpen(false);
            triggerRef.current?.focus();
        }
    };

    if (segmentsLength === 0) {
        return null;
    }

    let tooltipText = t('polish.title');
    if (isRetranscribing) {
        tooltipText = t('polish.retranscribing', 'Retranscribing...');
    } else if (isPolishing) {
        tooltipText = t('polish.polishing');
    }

    return (
        <div
            className={`export-menu ${className}`}
            ref={dropdownRef}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            style={{ display: 'inline-block', marginRight: '8px' }}
        >
            <button
                ref={triggerRef}
                id="polish-menu-button"
                className="btn btn-icon"
                onClick={() => setIsOpen(!isOpen)}
                disabled={isRetranscribing}
                aria-haspopup="true"
                aria-expanded={isOpen}
                aria-controls="polish-menu-dropdown"
                data-tooltip={tooltipText}
                data-tooltip-pos="bottom"
                aria-label={tooltipText}
            >
                {isRetranscribing ? (
                    <>
                        <ProcessingIcon />
                        <span style={{ fontSize: '0.75rem', fontWeight: 600 }}>{Math.floor(retranscribeProgress)}%</span>
                    </>
                ) : isPolishing ? (
                    <>
                        <ProcessingIcon />
                        <span style={{ fontSize: '0.75rem', fontWeight: 600 }}>{polishProgress}%</span>
                    </>
                ) : (
                    <SparklesIcon />
                )}
                <ChevronDownIcon />
            </button>

            {isOpen && (
                <div
                    ref={menuRef}
                    id="polish-menu-dropdown"
                    className={`export-dropdown position-${position}`}
                    role="menu"
                    aria-labelledby="polish-menu-button"
                >
                    {sourceHistoryId && sourceHistoryId !== 'current' && (
                        <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={handleRetranscribe}
                            disabled={isRetranscribing || isPolishing}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            <FileTextIcon />
                            <span>
                                {isRetranscribing
                                    ? t('polish.retranscribing', 'Retranscribing...')
                                    : t('polish.retranscribe', 'Re-transcribe')}
                            </span>
                        </button>
                    )}

                    <button
                        type="button"
                        className="export-dropdown-item"
                        onClick={handleStartPolish}
                        disabled={isPolishing || isRetranscribing}
                        role="menuitem"
                        tabIndex={-1}
                    >
                        <SparklesIcon />
                        <span>
                            {isPolishing
                                ? t('polish.polishing')
                                : t('polish.start', 'LLM Polish')}
                        </span>
                    </button>

                    {undoSegments && (
                        <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={handleUndoPolish}
                            disabled={isPolishing}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            <RestoreIcon />
                            <span>{t('polish.undo')}</span>
                        </button>
                    )}

                    {redoSegments && (
                        <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={handleRedoPolish}
                            disabled={isPolishing}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            <RedoIcon />
                            <span>{t('polish.redo')}</span>
                        </button>
                    )}

                    <div style={{ borderTop: '1px solid var(--color-border)', marginTop: '4px', paddingTop: '4px' }}>
                        <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                setShowAdvanced(!showAdvanced);
                            }}
                            role="menuitem"
                            tabIndex={-1}
                            style={{ justifyContent: 'space-between' }}
                        >
                            <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                {showAdvanced ? <ChevronDownIcon /> : <ChevronRightIcon />}
                                {t('polish.advanced_settings')}
                            </span>
                        </button>

                        {showAdvanced && (
                            <div style={{ padding: '0 12px 12px 12px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                                    <label style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                                        {t('polish.keywords')}
                                    </label>
                                    <input
                                        type="text"
                                        className="settings-input"
                                        style={{ fontSize: '0.8rem', padding: '4px 8px' }}
                                        placeholder={t('polish.keywords_placeholder')}
                                        value={config.polishKeywords || ''}
                                        onChange={(e) => setConfig({ polishKeywords: e.target.value })}
                                        onClick={(e) => e.stopPropagation()}
                                    />
                                </div>
                                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                                    <label style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                                        {t('polish.scenario_label')}
                                    </label>
                                    <select
                                        className="settings-input"
                                        style={{ fontSize: '0.8rem', padding: '4px 8px' }}
                                        value={config.polishScenario || 'custom'}
                                        onChange={(e) => setConfig({ polishScenario: e.target.value })}
                                        onClick={(e) => e.stopPropagation()}
                                    >
                                        <option value="customer_service">{t('polish.scenarios.customer_service')}</option>
                                        <option value="meeting">{t('polish.scenarios.meeting')}</option>
                                        <option value="interview">{t('polish.scenarios.interview')}</option>
                                        <option value="lecture">{t('polish.scenarios.lecture')}</option>
                                        <option value="podcast">{t('polish.scenarios.podcast')}</option>
                                        <option value="custom">{t('polish.scenarios.custom')}</option>
                                    </select>
                                </div>

                                {(config.polishScenario === 'custom' || !config.polishScenario) && (
                                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                                        <label style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                                            {t('polish.custom_context')}
                                        </label>
                                        <textarea
                                            className="settings-input"
                                            style={{ fontSize: '0.8rem', padding: '4px 8px', minHeight: '60px', resize: 'vertical' }}
                                            placeholder={t('polish.context_placeholder')}
                                            value={config.polishContext || ''}
                                            onChange={(e) => setConfig({ polishContext: e.target.value })}
                                            onClick={(e) => e.stopPropagation()}
                                        />
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
