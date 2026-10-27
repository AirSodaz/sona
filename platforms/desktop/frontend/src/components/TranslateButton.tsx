import type React from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LANGUAGE_OPTIONS } from '../constants/languages';
import {
  getActiveLlmConfig,
  getFeatureLlmConfig,
  isLlmConfigComplete,
} from '../services/llm/configUtils';
import { modelService } from '../services/modelService';
import { resolveItemPipeline } from '../services/projectPipeline';
import { toggleLiveTranslation } from '../services/tauri/recognizer';
import { translationService } from '../services/translationService';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { DEFAULT_LLM_STATE } from '../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { DEFAULT_PROJECT_PIPELINE } from '../types/project';
import { getLocalizedLanguageName } from '../utils/languageUtils';
import {
  CheckIcon,
  ChevronDownIcon,
  EditIcon,
  LanguagesIcon,
  PlayIcon,
  ProcessingIcon,
  ViewIcon,
  ViewOffIcon,
} from './Icons';

function SwapIcon(props?: React.SVGProps<SVGSVGElement>): React.JSX.Element {
  return (
    <svg
      aria-hidden="true"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <path d="m8 3 4 4-4 4" />
      <path d="M12 7H4" />
      <path d="m16 21-4-4 4-4" />
      <path d="M12 17h8" />
    </svg>
  );
}
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
export function TranslateButton({
  className = '',
}: TranslateButtonProps): React.JSX.Element | null {
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

  const closeMenu = useCallback(() => {
    setIsOpen(false);
    setSearchQuery('');
  }, []);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);

  // LLM state
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const llmState =
    useTranscriptSidecarStore((state) => state.llmStates[sourceHistoryId || 'current']) ||
    DEFAULT_LLM_STATE;
  const { isTranslating, translationProgress, isTranslationVisible, isRetranscribing } = llmState;
  const updateLlmState = useTranscriptSidecarStore((state) => state.updateLlmState);

  const config = useEffectiveConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const segments = useTranscriptSessionStore((state) => state.segments);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const projects = useProjectStore((state) => state.projects);
  const historyItems = useHistoryStore((state) => state.items);
  const currentItem =
    sourceHistoryId && sourceHistoryId !== 'current'
      ? historyItems.find((i) => i.id === sourceHistoryId)
      : null;
  const projectId = currentItem?.projectId ?? activeProjectId;
  const projectPipeline = resolveItemPipeline(projectId, projects, config);
  const effectiveTargetLang = projectPipeline.targetLanguage || config.translationLanguage || 'zh';
  const effectiveSourceLang = config.realtimeTranslationSourceLanguage || 'auto';
  const realtimeTranslationEnabled = config.realtimeTranslationEnabled ?? false;

  const [selectingMode, setSelectingMode] = useState<'source' | 'target'>('target');
  const switchSelectingMode = (mode: 'source' | 'target') => {
    setSelectingMode(mode);
    setSearchQuery('');
  };
  const [installedModels, setInstalledModels] = useState<Record<string, boolean>>({});

  const translationLlm = getFeatureLlmConfig(config, 'translation');
  const isLocalOnnx =
    translationLlm?.strategy === 'local_nmt' ||
    (!translationLlm && config.realtimeTranslationEngine === 'onnx');
  useEffect(() => {
    let active = true;
    if (isOpen && isLocalOnnx && typeof modelService?.isModelInstalled === 'function') {
      const pairs = ['opus-mt-en-zh', 'opus-mt-zh-en', 'opus-mt-ja-zh', 'opus-mt-zh-ja'];
      Promise.all(
        pairs.map(async (id) => {
          try {
            const installed = await modelService.isModelInstalled(id);
            return [id, installed] as const;
          } catch {
            return [id, false] as const;
          }
        })
      ).then((results) => {
        if (active) {
          setInstalledModels(Object.fromEntries(results));
        }
      });
    }
    return () => {
      active = false;
    };
  }, [isOpen, isLocalOnnx]);

  const hasTranslation = segments.some(
    (seg) => typeof seg.translation === 'string' && seg.translation.trim().length > 0
  );
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
  }, [isOpen, closeMenu]);

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
            (
              focusables[(currentIndex - 1 + focusables.length) % focusables.length] as HTMLElement
            ).focus();
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

    const llm = getFeatureLlmConfig(config, 'translation') ?? getActiveLlmConfig(config);
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
      await translationService.translateCurrentTranscript(effectiveTargetLang);
    } catch (error) {
      await showError({
        code: 'translation.failed',
        messageKey: 'errors.translation.failed',
        cause: error,
      });
    }
  };

  const handleToggleVisibility = () => {
    updateLlmState({ isTranslationVisible: !isTranslationVisible }, sourceHistoryId || undefined);
    closeMenu();
    triggerRef.current?.focus();
  };

  const handleSwapLanguages = () => {
    const defaultFallback = effectiveTargetLang === 'zh' ? 'en' : 'zh';
    const oldSrc = effectiveSourceLang === 'auto' ? defaultFallback : effectiveSourceLang;
    const oldTgt = effectiveTargetLang;
    const newSrc = oldTgt;
    let newTgt = oldSrc === newSrc ? defaultFallback : oldSrc;
    if (isLocalOnnx && (newSrc === 'en' || newSrc === 'ja')) {
      newTgt = 'zh';
    }
    setConfig({
      realtimeTranslationSourceLanguage: newSrc,
      realtimeTranslationTargetLanguage: newTgt,
      translationLanguage: newTgt,
    });

    if (projectId && projectPipeline.isProjectPipeline) {
      const project = useProjectStore.getState().projects.find((p) => p.id === projectId);
      if (project) {
        void useProjectStore.getState().updateProjectPipeline(projectId, {
          ...(project.pipeline ?? DEFAULT_PROJECT_PIPELINE),
          targetLanguage: newTgt,
        });
      }
    }

    if (isRecording && realtimeTranslationEnabled) {
      void toggleLiveTranslation({
        enabled: true,
        sourceLang: newSrc,
        targetLang: newTgt,
        catchupSegments: segments.filter((s) => s.isFinal),
      }).catch(async (error) => {
        await showError({
          code: 'translation.failed',
          messageKey: 'errors.translation.failed',
          cause: error,
        });
      });
    }
  };

  const handleToggleRealtime = async () => {
    const nextEnabled = !realtimeTranslationEnabled;
    setConfig({ realtimeTranslationEnabled: nextEnabled });
    try {
      await toggleLiveTranslation({
        enabled: nextEnabled,
        sourceLang: effectiveSourceLang,
        targetLang: effectiveTargetLang,
        catchupSegments: nextEnabled ? segments.filter((s) => s.isFinal) : null,
      });
    } catch (error) {
      setConfig({ realtimeTranslationEnabled: !nextEnabled });
      await showError({
        code: 'translation.failed',
        messageKey: 'errors.translation.failed',
        cause: error,
      });
    }
  };

  const handleSourceSelect = (langCode: string) => {
    let nextTarget = effectiveTargetLang;
    if (langCode === nextTarget) {
      nextTarget = langCode === 'zh' ? 'en' : 'zh';
    } else if (isLocalOnnx) {
      if (langCode === 'en' || langCode === 'ja') {
        nextTarget = 'zh';
      } else if (langCode === 'zh' && nextTarget === 'zh') {
        nextTarget = 'en';
      }
    }
    setConfig({
      realtimeTranslationSourceLanguage: langCode,
      realtimeTranslationTargetLanguage: nextTarget,
      translationLanguage: nextTarget,
    });
    if (isRecording && realtimeTranslationEnabled) {
      void toggleLiveTranslation({
        enabled: true,
        sourceLang: langCode,
        targetLang: nextTarget,
        catchupSegments: segments.filter((s) => s.isFinal),
      }).catch(async (error) => {
        await showError({
          code: 'translation.failed',
          messageKey: 'errors.translation.failed',
          cause: error,
        });
      });
    }
    switchSelectingMode('target');
  };

  const handleLanguageSelect = async (langCode: string) => {
    // Update state & localStorage for recently used languages
    setRecentLanguages((prev) => {
      const updated = [langCode, ...prev.filter((l) => l !== langCode)].slice(0, 3);
      try {
        localStorage.setItem('sona_recent_translation_languages', JSON.stringify(updated));
      } catch {
        // Quietly ignore storage errors
      }
      return updated;
    });
    if (projectId && projectPipeline.isProjectPipeline) {
      const project = useProjectStore.getState().projects.find((p) => p.id === projectId);
      if (project) {
        await useProjectStore.getState().updateProjectPipeline(projectId, {
          ...(project.pipeline ?? DEFAULT_PROJECT_PIPELINE),
          targetLanguage: langCode,
        });
      }
    }

    setConfig({
      translationLanguage: langCode,
      realtimeTranslationTargetLanguage: langCode,
    });

    if (isRecording && realtimeTranslationEnabled) {
      void toggleLiveTranslation({
        enabled: true,
        sourceLang: effectiveSourceLang,
        targetLang: langCode,
        catchupSegments: segments.filter((s) => s.isFinal),
      }).catch(async (error) => {
        await showError({
          code: 'translation.failed',
          messageKey: 'errors.translation.failed',
          cause: error,
        });
      });
    }

    closeMenu();
    triggerRef.current?.focus();
  };
  const commonLanguages = Array.from(
    new Set([...recentLanguages, ...STATIC_COMMON_LANGUAGES])
  ).slice(0, 10);

  const filteredLanguages = LANGUAGE_OPTIONS.filter((lang) => {
    const localized = getLocalizedLanguageName(lang.code, i18n.language);
    const search = searchQuery.toLowerCase();
    return (
      localized.toLowerCase().includes(search) ||
      lang.englishName.toLowerCase().includes(search) ||
      lang.code.toLowerCase().includes(search)
    );
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
        data-tooltip={
          isTranslating
            ? t('translation.translating')
            : t('translation.translate_all', { defaultValue: 'Translate Transcript' })
        }
        data-tooltip-pos="bottom"
        aria-label={
          isTranslating
            ? t('translation.translating')
            : t('translation.translate_all', { defaultValue: 'Translate Transcript' })
        }
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
          {isRecording ? (
            <div
              style={{
                padding: '8px 12px',
                margin: '4px 6px',
                background: 'var(--color-bg-secondary)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--color-border)',
              }}
            >
              <div
                style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
              >
                <div style={{ display: 'flex', flexDirection: 'column' }}>
                  <span style={{ fontSize: '0.85rem', fontWeight: 600 }}>
                    {t('translation.realtime_toggle', {
                      defaultValue: 'Real-time Bilingual Translation',
                    })}
                  </span>
                  <span style={{ fontSize: '0.7rem', color: 'var(--color-text-secondary)' }}>
                    {t('translation.realtime_toggle_desc', {
                      defaultValue: 'Follow speech in real-time · Auto catch-up recorded segments',
                    })}
                  </span>
                </div>
                <label className="toggle-switch" style={{ cursor: 'pointer', margin: 0 }}>
                  <input
                    type="checkbox"
                    checked={realtimeTranslationEnabled}
                    onChange={handleToggleRealtime}
                    role="switch"
                    aria-checked={realtimeTranslationEnabled}
                  />
                  <span className="toggle-slider round" />
                </label>
              </div>
            </div>
          ) : (
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
                  if (isTranslating)
                    return t('translation.translating', { defaultValue: 'Translating...' });
                  if (hasTranslation)
                    return t('translation.retranslate', { defaultValue: 'Retranslate' });
                  return t('translation.start', { defaultValue: 'Start Translation' });
                })()}
              </span>
            </button>
          )}
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
          <div
            style={{
              padding: '6px 12px 2px 12px',
              fontSize: '0.75rem',
              fontWeight: 600,
              color: 'var(--color-text-secondary)',
            }}
          >
            {selectingMode === 'source'
              ? t('translation.source_language', { defaultValue: 'Source Language' })
              : t('translation.target_language', { defaultValue: 'Target Language' })}
          </div>

          <div
            style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px 6px 8px' }}
          >
            <button
              type="button"
              onClick={() => switchSelectingMode('source')}
              style={{
                flex: 1,
                padding: '6px 8px',
                fontSize: '0.75rem',
                fontWeight: selectingMode === 'source' ? 600 : 400,
                background:
                  selectingMode === 'source'
                    ? 'var(--color-bg-hover)'
                    : 'var(--color-bg-secondary)',
                border:
                  selectingMode === 'source'
                    ? '1px solid var(--color-accent-primary)'
                    : '1px solid var(--color-border)',
                borderRadius: 'var(--radius-sm)',
                color: 'var(--color-text-primary)',
                cursor: 'pointer',
                textAlign: 'center',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {effectiveSourceLang === 'auto'
                ? t('translation.auto_detect', { defaultValue: 'Auto Detect' })
                : getLocalizedLanguageName(effectiveSourceLang, i18n.language)}
            </button>
            <button
              type="button"
              aria-label={t('translation.swap_languages', { defaultValue: 'Swap languages' })}
              title={t('translation.swap_languages', { defaultValue: 'Swap languages' })}
              onClick={handleSwapLanguages}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 28,
                height: 28,
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--color-border)',
                background: 'var(--color-bg-secondary)',
                color: 'var(--color-text-primary)',
                cursor: 'pointer',
                flexShrink: 0,
              }}
            >
              <SwapIcon />
            </button>
            <button
              type="button"
              onClick={() => switchSelectingMode('target')}
              style={{
                flex: 1,
                padding: '6px 8px',
                fontSize: '0.75rem',
                fontWeight: selectingMode === 'target' ? 600 : 400,
                background:
                  selectingMode === 'target'
                    ? 'var(--color-bg-hover)'
                    : 'var(--color-bg-secondary)',
                border:
                  selectingMode === 'target'
                    ? '1px solid var(--color-accent-primary)'
                    : '1px solid var(--color-border)',
                borderRadius: 'var(--radius-sm)',
                color: 'var(--color-text-primary)',
                cursor: 'pointer',
                textAlign: 'center',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {getLocalizedLanguageName(effectiveTargetLang, i18n.language)}
            </button>
          </div>
          {isLocalOnnx ? (
            <div style={{ maxHeight: '220px', overflowY: 'auto', padding: '4px 0' }}>
              {selectingMode === 'source'
                ? ['auto', 'zh', 'en', 'ja'].map((code) => {
                    const isSelected = effectiveSourceLang === code;
                    const label =
                      code === 'auto'
                        ? t('translation.auto_detect', { defaultValue: 'Auto Detect' })
                        : getLocalizedLanguageName(code, i18n.language);
                    return (
                      <button
                        key={code}
                        type="button"
                        className="export-dropdown-item"
                        onClick={() => handleSourceSelect(code)}
                        role="menuitem"
                        tabIndex={-1}
                        style={{ display: 'flex', alignItems: 'center', width: '100%', gap: '8px' }}
                      >
                        <span
                          style={{
                            width: 16,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            flexShrink: 0,
                          }}
                        >
                          {isSelected && <CheckIcon />}
                        </span>
                        <span>{label}</span>
                      </button>
                    );
                  })
                : (() => {
                    const candidates =
                      effectiveSourceLang === 'zh'
                        ? ['en', 'ja']
                        : effectiveSourceLang === 'en' || effectiveSourceLang === 'ja'
                          ? ['zh']
                          : ['zh', 'en', 'ja'];
                    return candidates.map((code) => {
                      const isSelected = effectiveTargetLang === code;
                      const resolvedSrc =
                        effectiveSourceLang === 'auto'
                          ? code === 'zh'
                            ? 'en'
                            : 'zh'
                          : effectiveSourceLang;
                      const pairModelId = `opus-mt-${resolvedSrc}-${code}`;
                      const isInstalled = installedModels[pairModelId] ?? true;
                      return (
                        <button
                          key={code}
                          type="button"
                          className="export-dropdown-item"
                          onClick={() => void handleLanguageSelect(code)}
                          role="menuitem"
                          tabIndex={-1}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            width: '100%',
                            gap: '8px',
                          }}
                        >
                          <span
                            style={{
                              width: 16,
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              flexShrink: 0,
                            }}
                          >
                            {isSelected && <CheckIcon />}
                          </span>
                          <span>{getLocalizedLanguageName(code, i18n.language)}</span>
                          {!isInstalled && (
                            <span
                              style={{
                                marginLeft: 'auto',
                                fontSize: '0.65rem',
                                padding: '1px 5px',
                                borderRadius: 4,
                                background: 'var(--color-bg-secondary)',
                                color: 'var(--color-text-secondary)',
                                border: '1px solid var(--color-border)',
                              }}
                            >
                              {t('translation.not_downloaded', { defaultValue: 'Not Downloaded' })}
                            </span>
                          )}
                        </button>
                      );
                    });
                  })()}
            </div>
          ) : selectingMode === 'source' ? (
            <div style={{ maxHeight: '220px', overflowY: 'auto', padding: '4px 0' }}>
              <button
                type="button"
                className="export-dropdown-item"
                onClick={() => handleSourceSelect('auto')}
                role="menuitem"
                tabIndex={-1}
                style={{ display: 'flex', alignItems: 'center', width: '100%', gap: '8px' }}
              >
                <span
                  style={{
                    width: 16,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    flexShrink: 0,
                  }}
                >
                  {effectiveSourceLang === 'auto' && <CheckIcon />}
                </span>
                <span>{t('translation.auto_detect', { defaultValue: 'Auto Detect' })}</span>
              </button>
              {filteredLanguages.map((lang) => {
                const isSelected = effectiveSourceLang === lang.code;
                return (
                  <button
                    key={lang.code}
                    type="button"
                    className="export-dropdown-item"
                    onClick={() => handleSourceSelect(lang.code)}
                    role="menuitem"
                    tabIndex={-1}
                    style={{ display: 'flex', alignItems: 'center', width: '100%', gap: '8px' }}
                  >
                    <span
                      style={{
                        width: 16,
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        flexShrink: 0,
                      }}
                    >
                      {isSelected && <CheckIcon />}
                    </span>
                    <span>{getLocalizedLanguageName(lang.code, i18n.language)}</span>
                  </button>
                );
              })}
            </div>
          ) : (
            <>
              <div style={{ padding: '4px 8px 8px 8px' }}>
                <input
                  type="text"
                  placeholder={t('translation.search_placeholder', {
                    defaultValue: 'Search languages...',
                  })}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => {
                    if (e.key === 'ArrowDown') {
                      e.preventDefault();
                      e.stopPropagation();
                      const firstCandidate = menuRef.current?.querySelector<HTMLButtonElement>(
                        '[data-candidate-lang="true"]'
                      );
                      firstCandidate?.focus();
                    } else if (e.key === 'Escape') {
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
                  <div
                    style={{
                      fontSize: '0.7rem',
                      fontWeight: 600,
                      color: 'var(--color-text-secondary)',
                      marginBottom: '6px',
                      textTransform: 'uppercase',
                      letterSpacing: '0.05em',
                    }}
                  >
                    {t('translation.commonly_used', { defaultValue: 'Commonly Used' })}
                  </div>
                  <div
                    style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '4px' }}
                  >
                    {commonLanguages.map((lang) => {
                      const isSelected = effectiveTargetLang === lang;
                      return (
                        <button
                          key={`common-${lang}`}
                          type="button"
                          data-candidate-lang="true"
                          onClick={() => void handleLanguageSelect(lang)}
                          style={{
                            padding: '4px 6px',
                            fontSize: '0.75rem',
                            background: isSelected
                              ? 'var(--color-bg-hover)'
                              : 'var(--color-bg-secondary)',
                            border: isSelected
                              ? '1px solid var(--color-accent-primary)'
                              : '1px solid var(--color-border)',
                            borderRadius: 'var(--radius-sm)',
                            color: isSelected
                              ? 'var(--color-text-primary)'
                              : 'var(--color-text-secondary)',
                            cursor: 'pointer',
                            textAlign: 'center',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                            fontWeight: isSelected ? '600' : 'normal',
                            transition: 'all 0.15s ease',
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
                <div
                  style={{
                    padding: '4px 12px 6px 12px',
                    fontSize: '0.7rem',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                  }}
                >
                  {searchQuery
                    ? t('translation.search_results', { defaultValue: 'Search Results' })
                    : t('translation.all_languages', { defaultValue: 'All Languages' })}
                </div>
                <div style={{ maxHeight: '180px', overflowY: 'auto', padding: '2px 0' }}>
                  {filteredLanguages.length === 0 ? (
                    <div
                      style={{
                        padding: '8px 12px',
                        fontSize: '0.8rem',
                        color: 'var(--color-text-muted)',
                        textAlign: 'center',
                      }}
                    >
                      {t('translation.no_results', { defaultValue: 'No languages found' })}
                    </div>
                  ) : (
                    filteredLanguages.map((lang) => {
                      const isSelected = effectiveTargetLang === lang.code;
                      return (
                        <button
                          key={lang.code}
                          type="button"
                          className="export-dropdown-item"
                          onClick={() => void handleLanguageSelect(lang.code)}
                          data-candidate-lang="true"
                          role="menuitem"
                          tabIndex={-1}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            width: '100%',
                            gap: '8px',
                          }}
                        >
                          <span
                            style={{
                              width: 16,
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              flexShrink: 0,
                            }}
                          >
                            {isSelected && <CheckIcon />}
                          </span>
                          <span>{getLocalizedLanguageName(lang.code, i18n.language)}</span>
                        </button>
                      );
                    })
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
