import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { polishService } from '../services/polishService';
import { SparklesIcon, ChevronDownIcon, ChevronRightIcon, ProcessingIcon, RestoreIcon, RedoIcon } from './Icons';
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
    const isPolishing = useTranscriptStore((state) => state.isPolishing);
    const polishProgress = useTranscriptStore((state) => state.polishProgress);
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

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                const nextIndex = (currentIndex + 1) % buttons.length;
                buttons[nextIndex].focus();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                const prevIndex = (currentIndex - 1 + buttons.length) % buttons.length;
                buttons[prevIndex].focus();
            } else if (e.key === 'Home') {
                e.preventDefault();
                buttons[0].focus();
            } else if (e.key === 'End') {
                e.preventDefault();
                buttons[buttons.length - 1].focus();
            }
        }
    };

    const handleStartPolish = async () => {
        if (isPolishing) return;

        if (!config.aiApiKey || !config.aiBaseUrl || !config.aiModel) {
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
                aria-haspopup="true"
                aria-expanded={isOpen}
                aria-controls="polish-menu-dropdown"
                data-tooltip={isPolishing ? t('polish.polishing') : t('polish.title')}
                data-tooltip-pos="bottom"
            >
                {isPolishing ? (
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
                    <button
                        type="button"
                        className="export-dropdown-item"
                        onClick={handleStartPolish}
                        disabled={isPolishing}
                        role="menuitem"
                        tabIndex={-1}
                    >
                        <SparklesIcon />
                        <span>
                            {isPolishing
                                ? t('polish.polishing')
                                : t('polish.start')}
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
                                        {t('polish.context')}
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
                            </div>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
