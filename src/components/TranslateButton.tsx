import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { translationService } from '../services/translationService';
import { LanguagesIcon, ChevronDownIcon, PlayIcon, ViewIcon, ViewOffIcon, ProcessingIcon, EditIcon, CheckIcon } from './Icons';

const LANGUAGES = ['zh', 'en', 'ja', 'ko', 'fr', 'de', 'es'];

/** Props for TranslateButton. */
interface TranslateButtonProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Dropdown button component for translating transcript segments.
 *
 * @param props - Component props.
 * @return The translate button component.
 */
export function TranslateButton({ className = '' }: TranslateButtonProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const { alert } = useDialogStore();
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const dropdownRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    const segmentsLength = useTranscriptStore((state) => state.segments.length);

    // LLM state
    const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
    const llmState = useTranscriptStore((state) => state.llmStates[sourceHistoryId || 'current']) || { isTranslating: false, translationProgress: 0, isTranslationVisible: false, isRetranscribing: false };
    const { isTranslating, translationProgress, isTranslationVisible, isRetranscribing } = llmState;
    const toggleTranslationVisible = useTranscriptStore((state) => state.setIsTranslationVisible);

    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const segments = useTranscriptStore((state) => state.segments);

    const hasTranslation = segments.some(seg => typeof seg.translation === 'string' && seg.translation.trim().length > 0);

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
            const estimatedHeight = 150;
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

    // Focus management when opening
    useEffect(() => {
        if (isOpen && menuRef.current) {
            const firstButton = menuRef.current.querySelector('button');
            if (firstButton) {
                requestAnimationFrame(() => firstButton.focus());
            }
        }
    }, [isOpen]);

    const handleBlur = (e: React.FocusEvent) => {
        // Close menu if focus leaves the component
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

    const handleStartTranslation = async () => {
        if (isTranslating) return;

        if (!config.llmApiKey || !config.llmBaseUrl || !config.llmModel) {
            await alert(t('translation.error_config_missing', { defaultValue: 'Please configure LLM service in Settings before translating.' }), { variant: 'error' });
            return;
        }

        setIsOpen(false);
        triggerRef.current?.focus();

        try {
            await translationService.translateCurrentTranscript();
        } catch (error: any) {
            await alert(t('translation.error_failed', { defaultValue: 'Translation failed: ' }) + (error.message || 'Unknown error'), { variant: 'error' });
        }
    };

    const handleToggleVisibility = () => {
        toggleTranslationVisible(!isTranslationVisible);
        setIsOpen(false);
        triggerRef.current?.focus();
    };

    const handleLanguageSelect = (langCode: string) => {
        setConfig({ translationLanguage: langCode });
        // Don't close menu to allow quick translation start after selection?
        // Or close it to indicate selection made. Let's close it.
        setIsOpen(false);
        triggerRef.current?.focus();
    };

    // Only show if there's transcript content
    if (segmentsLength === 0) {
        return null;
    }

    return (
        <div
            className={`export-menu ${className}`} // Reuse export-menu styles for consistency
            ref={dropdownRef}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            style={{ display: 'inline-block' }}
        >
            <button
                ref={triggerRef}
                id="translate-menu-button"
                className="btn btn-icon"
                onClick={() => setIsOpen(!isOpen)}
                disabled={isRetranscribing}
                aria-haspopup="true"
                aria-expanded={isOpen}
                aria-controls="translate-menu-dropdown"
                data-tooltip={isTranslating ? t('translation.translating') : t('translation.translate_all', { defaultValue: 'Translate Transcript' })}
                data-tooltip-pos="bottom"
                aria-label={isTranslating ? t('translation.translating') : t('translation.translate_all', { defaultValue: 'Translate Transcript' })}
            >
                {isTranslating ? (
                    <>
                        <ProcessingIcon />
                        <span style={{ fontSize: '0.75rem', fontWeight: 600 }}>{translationProgress}%</span>
                    </>
                ) : (
                    <LanguagesIcon />
                )}
                <ChevronDownIcon />
            </button>

            {isOpen && (
                <div
                    ref={menuRef}
                    id="translate-menu-dropdown"
                    className={`export-dropdown position-${position}`} // Reuse export-dropdown styles
                    role="menu"
                    aria-labelledby="translate-menu-button"
                >
                    <button
                        type="button"
                        className="export-dropdown-item"
                        onClick={handleStartTranslation}
                        disabled={isTranslating}
                        role="menuitem"
                        tabIndex={-1}
                    >
                        {hasTranslation ? <EditIcon /> : <PlayIcon />}
                        <span>
                            {(() => {
                                if (isTranslating) return t('translation.translating', { defaultValue: 'Translating...' });
                                if (hasTranslation) return t('translation.retranslate', { defaultValue: 'Retranslate' });
                                return t('translation.start', { defaultValue: 'Start Translation' });
                            })()}
                        </span>
                    </button>
                    <button
                        type="button"
                        className="export-dropdown-item"
                        onClick={handleToggleVisibility}
                        role="menuitem"
                        tabIndex={-1}
                    >
                        {isTranslationVisible ? <ViewOffIcon /> : <ViewIcon />}
                        <span>
                            {isTranslationVisible
                                ? t('translation.hide_bilingual', { defaultValue: 'Hide Translations' })
                                : t('translation.show_bilingual', { defaultValue: 'Show Translations' })}
                        </span>
                    </button>

                    <div style={{ height: 1, background: 'var(--color-border)', margin: '4px 0' }} />
                    <div style={{ padding: '8px 12px', fontSize: '0.75rem', fontWeight: 600, color: 'var(--color-text-secondary)' }}>
                        {t('translation.target_language', { defaultValue: 'Target Language' })}
                    </div>

                    {LANGUAGES.map((lang) => (
                        <button
                            key={lang}
                            type="button"
                            className="export-dropdown-item"
                            onClick={() => handleLanguageSelect(lang)}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            <span style={{ width: 16, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                                {(config.translationLanguage || 'zh') === lang && <CheckIcon />}
                            </span>
                            <span>{t(`translation.languages.${lang}`)}</span>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
