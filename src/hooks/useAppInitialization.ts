import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore, useUIConfig } from '../stores/configStore';
import i18n from '../i18n';
import { ensureLlmState } from '../services/llmConfig';

/**
 * Hook to handle application initialization.
 *
 * - Loads configuration from localStorage.
 * - Applies theme settings.
 * - Applies font settings.
 * - Persists config changes (debounced).
 */
export function useAppInitialization() {
    const config = useConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const [isLoaded, setIsLoaded] = useState(false);

    // Domain-specific selectors for fine-grained dependency tracking
    const { theme, font, minimizeToTrayOnExit } = useUIConfig();

    // Initialize config from localStorage
    useEffect(() => {
        const saved = localStorage.getItem('sona-config');
        if (saved) {
            try {
                const parsed = JSON.parse(saved);
                // Check if valid config object
                if (parsed.streamingModelPath || parsed.offlineModelPath || parsed.recognitionModelPath || parsed.modelPath || parsed.appLanguage || parsed.language || parsed.llmSettings || parsed.llm) {
                    const { llmSettings } = ensureLlmState(parsed);
                    const loadedConfig = {
                        streamingModelPath: parsed.streamingModelPath || parsed.recognitionModelPath || parsed.offlineModelPath || parsed.modelPath || '',
                        offlineModelPath: parsed.offlineModelPath || parsed.recognitionModelPath || parsed.modelPath || '',
                        punctuationModelPath: parsed.punctuationModelPath || '',
                        vadModelPath: parsed.vadModelPath || '',
                        enabledITNModels: parsed.enabledITNModels || (parsed.enableITN ? ['itn-zh-number'] : []),
                        itnRulesOrder: parsed.itnRulesOrder || ['itn-zh-number'],
                        enableITN: parsed.enableITN ?? ((parsed.enabledITNModels?.length ?? 0) > 0),
                        vadBufferSize: parsed.vadBufferSize || 5,
                        maxConcurrent: parsed.maxConcurrent || 2,
                        appLanguage: parsed.appLanguage || 'auto',
                        theme: parsed.theme || 'auto',
                        font: parsed.font || 'system',
                        language: parsed.language || 'auto',
                        enableTimeline: parsed.enableTimeline ?? false,
                        minimizeToTrayOnExit: parsed.minimizeToTrayOnExit ?? true,
                        lockWindow: parsed.lockWindow ?? false,
                        alwaysOnTop: parsed.alwaysOnTop ?? true,
                        microphoneId: parsed.microphoneId || 'default',
                        microphoneBoost: parsed.microphoneBoost ?? 1.0,
                        systemAudioDeviceId: parsed.systemAudioDeviceId || 'default',
                        muteDuringRecording: parsed.muteDuringRecording ?? false,
                        startOnLaunch: parsed.startOnLaunch ?? false,
                        captionWindowWidth: parsed.captionWindowWidth || 800,
                        captionFontSize: parsed.captionFontSize || 24,
                        captionFontColor: parsed.captionFontColor || '#ffffff',
                        llmSettings,
                        translationLanguage: parsed.translationLanguage || 'zh',
                        polishKeywords: parsed.polishKeywords || '',
                        polishContext: parsed.polishContext || '',
                        polishScenario: parsed.polishScenario || '',
                        autoPolish: parsed.autoPolish ?? false,
                        autoPolishFrequency: parsed.autoPolishFrequency || 5,
                        autoCheckUpdates: parsed.autoCheckUpdates ?? true,
                    };

                    setConfig(loadedConfig);

                    // Auto-start caption mode if configured
                    if (loadedConfig.startOnLaunch) {
                        setIsCaptionMode(true);
                    }

                    // Apply language immediately
                    if (parsed.appLanguage && parsed.appLanguage !== 'auto') {
                        i18n.changeLanguage(parsed.appLanguage);
                    } else {
                        i18n.changeLanguage(navigator.language);
                    }
                }
            } catch (e) {
                console.error('Failed to parse saved config:', e);
            }
        }
        setIsLoaded(true);
    }, [setConfig]);

    // Apply theme
    useEffect(() => {
        const currentTheme = theme || 'auto';
        const root = document.documentElement;

        const applyTheme = (targetTheme: string) => {
            switch (targetTheme) {
                case 'dark':
                case 'light':
                    root.setAttribute('data-theme', targetTheme);
                    break;
                default:
                    root.removeAttribute('data-theme');
                    break;
            }
        };

        if (currentTheme === 'auto') {
            const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

            // Initial check
            applyTheme(mediaQuery.matches ? 'dark' : 'light');

            // Listen for changes
            const handleChange = (e: MediaQueryListEvent) => {
                applyTheme(e.matches ? 'dark' : 'light');
            };

            mediaQuery.addEventListener('change', handleChange);
            return () => mediaQuery.removeEventListener('change', handleChange);
        } else {
            applyTheme(currentTheme);
        }
    }, [theme]);

    // Persist config changes to localStorage (debounced)
    useEffect(() => {
        if (!isLoaded) return;

        const timeoutId = setTimeout(() => {
            localStorage.setItem('sona-config', JSON.stringify(config));
        }, 500);

        return () => clearTimeout(timeoutId);
    }, [config, isLoaded]);

    // Sync minimize to tray setting with backend
    useEffect(() => {
        if (!isLoaded) return;
        invoke('set_minimize_to_tray', { enabled: minimizeToTrayOnExit ?? true })
            .catch(e => console.error('Failed to set minimize to tray:', e));
    }, [minimizeToTrayOnExit, isLoaded]);

    // Apply font
    useEffect(() => {
        const currentFont = font || 'system';
        const root = document.documentElement;

        const setFontVars = (fontFamily: string) => {
            root.style.setProperty('--font-sans', fontFamily);
            root.style.setProperty('--font-serif', fontFamily);
            root.style.setProperty('--font-mono', fontFamily);
        };

        const removeFontVars = () => {
            root.style.removeProperty('--font-sans');
            root.style.removeProperty('--font-serif');
            root.style.removeProperty('--font-mono');
        };

        switch (currentFont) {
            case 'serif':
                setFontVars('Merriweather, serif');
                break;
            case 'sans':
                setFontVars('Inter, sans-serif');
                break;
            case 'mono':
                setFontVars('JetBrains Mono, monospace');
                break;
            case 'arial':
                setFontVars('Arial, sans-serif');
                break;
            case 'georgia':
                setFontVars('Georgia, serif');
                break;
            case 'system':
            default:
                removeFontVars();
                break;
        }
    }, [font]);
}
