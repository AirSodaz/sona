import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';
import { useDialogStore } from '../stores/dialogStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { translationService } from '../services/translationService';
import { LanguagesIcon, ChevronDownIcon, PlayIcon, ViewIcon, ViewOffIcon, ProcessingIcon, EditIcon, CheckIcon } from './Icons';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../services/llm/configUtils';
import { LANGUAGE_OPTIONS } from '../constants/languages';
import { getLocalizedLanguageName } from '../utils/languageUtils';

const STATIC_COMMON_LANGUAGES = ['zh', 'en', 'ja', 'ko', 'fr', 'de', 'es', 'pt', 'ru', 'it'];

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
    const { t, i18n } = useTranslation();
    const showError = useDialogStore((state) => state.showError);
    const [isOpen, setIsOpen] = useState(false);
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const [searchQuery, setSearchQuery] = useState('');
    const [recentLanguages, setRecentLanguages] = useState<string[]>(() => {
        try {
            const stored = localStorage.getItem('sona_recent_translation_languages');
            if (stored) {
                const parsed = JSON.parse(stored);
                if (Array.isArray(parsed)) {
                    return parsed.filter((l): l is string => typeof l === 'string');
                }
            }
        } catch {
            // Quietly ignore storage errors
        }
        return [];
    });

    const closeMenu = () => {
        setIsOpen(false);
        setSearchQuery('');
    };
    const dropdownRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);

    // LLM state
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const llmState = useTranscriptSidecarStore((state) => state.llmStates[sourceHistoryId || 'current']) || { isTranslating: false, translationProgress: 0, isTranslationVisible: false, isRetranscribing: false };
    const { isTranslating, translationProgress, isTranslationVisible, isRetranscribing } = llmState;
    const updateLlmState = useTranscriptSidecarStore((state) => state.updateLlmState);

    const config = useEffectiveConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);
    const activeProjectId = useProjectStore((state) => state.activeProjectId);
    const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);
    const segments = useTranscriptSessionStore((state) => state.segments);

    const hasTranslation = segments.some(seg => typeof seg.translation === 'string' && seg.translation.trim().length > 0);

    // Close dropdown when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                closeMenu();
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

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isOpen) return;

        if (e.key === 'Escape') {
            e.preventDefault();
            closeMenu();
            triggerRef.current?.focus();
            return;
        }

        const activeEl = document.activeElement;
        const isInputActive = activeEl instanceof HTMLInputElement;

        if (menuRef.current) {
            const focusables = Array.from(menuRef.current.querySelectorAll('button, input'));
            const currentIndex = focusables.indexOf(activeEl as HTMLElement);

            if (currentIndex !== -1) {
                switch (e.key) {
                    case 'ArrowDown':
                        e.preventDefault();
                        (focusables[(currentIndex + 1) % focusables.length] as HTMLElement).focus();
                        break;
                    case 'ArrowUp':
                        e.preventDefault();
                        (focusables[(currentIndex - 1 + focusables.length) % focusables.length] as HTMLElement).focus();
                        break;
                    case 'Home':
                        if (!isInputActive) {
                            e.preventDefault();
                            (focusables[0] as HTMLElement).focus();
                        }
                        break;
                    case 'End':
                        if (!isInputActive) {
                            e.preventDefault();
                            (focusables[focusables.length - 1] as HTMLElement).focus();
                        }
                        break;
                }
            }
        }
    };

    const handleStartTranslation = async () => {
        if (isTranslating) return;

        const llm = getFeatureLlmConfig(config, 'translation');
        if (!isLlmConfigComplete(llm)) {
            await showError({
                code: 'config.translation_model_missing',
                messageKey: 'errors.config.translation_model_missing',
                showCause: false,
            });
            return;
        }

        closeMenu();
        triggerRef.current?.focus();

        try {
            await translationService.translateCurrentTranscript();
        } catch (error) {
            await showError({
                code: 'translation.failed',
                messageKey: 'errors.translation.failed',
                cause: error,
            });
        }
    };

    const handleToggleVisibility = () => {
        updateLlmState({ isTranslationVisible: !isTranslationVisible });
        closeMenu();
        triggerRef.current?.focus();
    };

    const handleLanguageSelect = async (langCode: string) => {
        // Update state & localStorage for recently used languages
        setRecentLanguages(prev => {
            const updated = [langCode, ...prev.filter(l => l !== langCode)].slice(0, 3);
            try {
                localStorage.setItem('sona_recent_translation_languages', JSON.stringify(updated));
            } catch {
                // Quietly ignore storage errors
            }
            return updated;
        });

        if (activeProjectId) {
            await updateProjectDefaults(activeProjectId, { translationLanguage: langCode });
        } else {
            setConfig({ translationLanguage: langCode });
        }
        // Don't close menu to allow quick translation start after selection?
        // Or close it to indicate selection made. Let's close it.
        closeMenu();
        triggerRef.current?.focus();
    };

    const commonLanguages = Array.from(new Set([...recentLanguages, ...STATIC_COMMON_LANGUAGES])).slice(0, 10);

    const filteredLanguages = LANGUAGE_OPTIONS.filter(lang => {
        const localized = getLocalizedLanguageName(lang.code, i18n.language);
        const search = searchQuery.toLowerCase();
        return localized.toLowerCase().includes(search) ||
               lang.englishName.toLowerCase().includes(search) ||
               lang.code.toLowerCase().includes(search);
    });

    // Only show if there's transcript content
    if (segmentsLength === 0) {
        return null;
    }

    return (
        <div
            className={`export-menu ${className}`} // Reuse export-menu styles for consistency
            ref={dropdownRef}
            onKeyDown={handleKeyDown}
            style={{ display: 'inline-block' }}
        >
            <button
                ref={triggerRef}
                id="translate-menu-button"
                className="btn btn-icon"
                onClick={() => {
                    if (isOpen) {
                        closeMenu();
                    } else {
                        setIsOpen(true);
                    }
                }}
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
                    style={{ minWidth: '250px' }}
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
                    <div style={{ padding: '8px 12px 4px 12px', fontSize: '0.75rem', fontWeight: 600, color: 'var(--color-text-secondary)' }}>
                        {t('translation.target_language', { defaultValue: 'Target Language' })}
                    </div>

                    <div style={{ padding: '4px 8px 8px 8px' }}>
                        <input
                            type="text"
                            placeholder={t('translation.search_placeholder', { defaultValue: 'Search languages...' })}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            onClick={(e) => e.stopPropagation()}
                            onKeyDown={(e) => {
                                if (e.key === 'ArrowDown') {
                                    e.preventDefault();
                                    if (menuRef.current) {
                                        const buttons = Array.from(menuRef.current.querySelectorAll('button'));
                                        if (buttons.length > 2) {
                                            buttons[2].focus();
                                        }
                                    }
                                } else if (e.key === 'Escape') {
                                    e.preventDefault();
                                    closeMenu();
                                    triggerRef.current?.focus();
                                } else {
                                    e.stopPropagation();
                                }
                            }}
                            style={{
                                width: '100%',
                                padding: '6px 8px',
                                fontSize: '0.8rem',
                                border: '1px solid var(--color-border)',
                                borderRadius: 'var(--radius-md)',
                                background: 'var(--color-bg-primary)',
                                color: 'var(--color-text-primary)',
                                outline: 'none',
                                boxSizing: 'border-box',
                            }}
                        />
                    </div>

                    {!searchQuery && (
                        <div style={{ padding: '0 8px 8px 8px' }}>
                            <div style={{ fontSize: '0.7rem', fontWeight: 600, color: 'var(--color-text-secondary)', marginBottom: '6px', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                                {t('translation.commonly_used', { defaultValue: 'Commonly Used' })}
                            </div>
                            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '4px' }}>
                                {commonLanguages.map((lang) => {
                                    const isSelected = (config.translationLanguage || 'zh') === lang;
                                    return (
                                        <button
                                            key={`common-${lang}`}
                                            type="button"
                                            onClick={() => void handleLanguageSelect(lang)}
                                            style={{
                                                padding: '4px 6px',
                                                fontSize: '0.75rem',
                                                background: isSelected ? 'var(--color-bg-hover)' : 'var(--color-bg-secondary)',
                                                border: isSelected ? '1px solid var(--color-accent-primary)' : '1px solid var(--color-border)',
                                                borderRadius: 'var(--radius-sm)',
                                                color: isSelected ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
                                                cursor: 'pointer',
                                                textAlign: 'center',
                                                overflow: 'hidden',
                                                textOverflow: 'ellipsis',
                                                whiteSpace: 'nowrap',
                                                fontWeight: isSelected ? '600' : 'normal',
                                                transition: 'all 0.15s ease',
                                            }}
                                            onMouseEnter={(e) => {
                                                e.currentTarget.style.background = 'var(--color-bg-hover)';
                                                e.currentTarget.style.color = 'var(--color-text-primary)';
                                            }}
                                            onMouseLeave={(e) => {
                                                if (!isSelected) {
                                                    e.currentTarget.style.background = 'var(--color-bg-secondary)';
                                                    e.currentTarget.style.color = 'var(--color-text-secondary)';
                                                }
                                            }}
                                            title={getLocalizedLanguageName(lang, i18n.language)}
                                        >
                                            {getLocalizedLanguageName(lang, i18n.language)}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: '6px' }}>
                        <div style={{ padding: '4px 12px 6px 12px', fontSize: '0.7rem', fontWeight: 600, color: 'var(--color-text-secondary)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                            {searchQuery
                                ? t('translation.search_results', { defaultValue: 'Search Results' })
                                : t('translation.all_languages', { defaultValue: 'All Languages' })}
                        </div>
                        <div style={{ maxHeight: '180px', overflowY: 'auto', padding: '2px 0' }}>
                            {filteredLanguages.length === 0 ? (
                                <div style={{ padding: '8px 12px', fontSize: '0.8rem', color: 'var(--color-text-muted)', textAlign: 'center' }}>
                                    {t('translation.no_results', { defaultValue: 'No languages found' })}
                                </div>
                            ) : (
                                filteredLanguages.map((lang) => {
                                    const isSelected = (config.translationLanguage || 'zh') === lang.code;
                                    return (
                                        <button
                                            key={lang.code}
                                            type="button"
                                            className="export-dropdown-item"
                                            onClick={() => void handleLanguageSelect(lang.code)}
                                            role="menuitem"
                                            tabIndex={-1}
                                            style={{ display: 'flex', alignItems: 'center', width: '100%', gap: '8px' }}
                                        >
                                            <span style={{ width: 16, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                                                {isSelected && <CheckIcon />}
                                            </span>
                                            <span>{getLocalizedLanguageName(lang.code, i18n.language)}</span>
                                        </button>
                                    );
                                })
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
