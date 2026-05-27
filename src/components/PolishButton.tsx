import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { SparklesIcon, ChevronDownIcon, ProcessingIcon, RestoreIcon, RedoIcon, FileTextIcon, SettingsIcon } from './Icons';
import { isHistoryItemDraft } from '../types/history';
import { PolishSettingsModal } from './PolishSettingsModal';
import { usePolishActions } from '../hooks/usePolishActions';

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
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    const {
        isPolishing,
        polishProgress,
        isRetranscribing,
        retranscribeProgress,
        undoSegments,
        redoSegments,
        handleStartPolish,
        handleRetranscribe,
        handleUndoPolish,
        handleRedoPolish,
    } = usePolishActions();

    const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);

    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const currentHistoryItem = useHistoryStore((state) => (
        sourceHistoryId ? state.items.find((item) => item.id === sourceHistoryId) || null : null
    ));
    const canRetranscribeCurrentHistory = !!currentHistoryItem && !isHistoryItemDraft(currentHistoryItem);

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

    const onActionStart = () => {
        setIsOpen(false);
        triggerRef.current?.focus();
    };

    const onRetranscribeClick = () => {
        handleRetranscribe(onActionStart);
    };

    const onStartPolishClick = () => {
        handleStartPolish(onActionStart);
    };

    const onUndoPolishClick = () => {
        handleUndoPolish(onActionStart);
    };

    const onRedoPolishClick = () => {
        handleRedoPolish(onActionStart);
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
                {(() => {
                    if (isRetranscribing) {
                        return (
                            <>
                                <ProcessingIcon />
                                <span style={{ fontSize: '0.75rem', fontWeight: 600 }}>{Math.floor(retranscribeProgress)}%</span>
                            </>
                        );
                    }
                    if (isPolishing) {
                        return (
                            <>
                                <ProcessingIcon />
                                <span style={{ fontSize: '0.75rem', fontWeight: 600 }}>{polishProgress}%</span>
                            </>
                        );
                    }
                    return <SparklesIcon />;
                })()}
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
                    {sourceHistoryId && sourceHistoryId !== 'current' && canRetranscribeCurrentHistory && (
                        <button
                            type="button"
                            className="export-dropdown-item"
                            onClick={onRetranscribeClick}
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
                        onClick={onStartPolishClick}
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
                            onClick={onUndoPolishClick}
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
                            onClick={onRedoPolishClick}
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
                                setIsSettingsOpen(true);
                                setIsOpen(false);
                            }}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            <SettingsIcon />
                            <span>{t('polish.advanced_settings')}</span>
                        </button>
                    </div>
                </div>
            )}

            <PolishSettingsModal 
                isOpen={isSettingsOpen} 
                onClose={() => setIsSettingsOpen(false)} 
            />
        </div>
    );
}
