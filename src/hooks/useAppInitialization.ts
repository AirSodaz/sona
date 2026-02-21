import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import i18n from '../i18n';

/**
 * Hook to handle application initialization.
 *
 * - Loads configuration from localStorage.
 * - Applies theme settings.
 * - Applies font settings.
 */
export function useAppInitialization() {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const [isLoaded, setIsLoaded] = useState(false);

    // Initialize config from localStorage
    useEffect(() => {
        const saved = localStorage.getItem('sona-config');
        if (saved) {
            try {
                const parsed = JSON.parse(saved);
                // Check if valid config object
                if (parsed.offlineModelPath || parsed.modelPath || parsed.appLanguage || parsed.language) {
                    setConfig({
                        offlineModelPath: parsed.offlineModelPath || parsed.modelPath || '',
                        punctuationModelPath: parsed.punctuationModelPath || '',
                        vadModelPath: parsed.vadModelPath || '',
                        ctcModelPath: parsed.ctcModelPath || '',
                        enabledITNModels: parsed.enabledITNModels || (parsed.enableITN ? ['itn-zh-number'] : []),
                        itnRulesOrder: parsed.itnRulesOrder || ['itn-zh-number'],
                        enableITN: parsed.enableITN ?? ((parsed.enabledITNModels?.length ?? 0) > 0),
                        vadBufferSize: parsed.vadBufferSize || 5,
                        maxConcurrent: parsed.maxConcurrent || 2,
                        appLanguage: parsed.appLanguage || 'auto',
                        theme: parsed.theme || 'auto',
                        font: parsed.font || 'system',
                        language: parsed.language || 'auto',
                        enableTimeline: parsed.enableTimeline ?? true,
                        minimizeToTrayOnExit: parsed.minimizeToTrayOnExit ?? true,
                        muteDuringRecording: parsed.muteDuringRecording ?? false
                    });

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
        const theme = config.theme || 'auto';
        const root = document.documentElement;

        const applyTheme = (targetTheme: string) => {
            if (targetTheme === 'dark') {
                root.setAttribute('data-theme', 'dark');
            } else if (targetTheme === 'light') {
                root.setAttribute('data-theme', 'light');
            } else {
                root.removeAttribute('data-theme');
            }
        };

        if (theme === 'auto') {
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
            applyTheme(theme);
        }
    }, [config.theme]);

    // Persist config changes
    useEffect(() => {
        if (!isLoaded) return;

        // Debounce could be added here if needed, but config changes are infrequent.
        const configToSave = {
            offlineModelPath: config.offlineModelPath,
            punctuationModelPath: config.punctuationModelPath,
            vadModelPath: config.vadModelPath,
            ctcModelPath: config.ctcModelPath,
            vadBufferSize: config.vadBufferSize,
            maxConcurrent: config.maxConcurrent,
            enabledITNModels: config.enabledITNModels,
            itnRulesOrder: config.itnRulesOrder,
            enableITN: config.enableITN,
            appLanguage: config.appLanguage,
            theme: config.theme,
            font: config.font,
            language: config.language,
            enableTimeline: config.enableTimeline,
            minimizeToTrayOnExit: config.minimizeToTrayOnExit,
            muteDuringRecording: config.muteDuringRecording
        };
        localStorage.setItem('sona-config', JSON.stringify(configToSave));
    }, [config, isLoaded]);

    // Sync minimize to tray setting with backend
    useEffect(() => {
        if (!isLoaded) return;
        invoke('set_minimize_to_tray', { enabled: config.minimizeToTrayOnExit ?? true })
            .catch(e => console.error('Failed to set minimize to tray:', e));
    }, [config.minimizeToTrayOnExit, isLoaded]);

    // Apply font
    useEffect(() => {
        const font = config.font || 'system';
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

        switch (font) {
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
    }, [config.font]);
}
