import { useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import i18n from '../i18n';

/**
 * Sets the font CSS variables on the root element.
 */
function setFontVars(root: HTMLElement, fontFamily: string) {
    root.style.setProperty('--font-sans', fontFamily);
    root.style.setProperty('--font-serif', fontFamily);
    root.style.setProperty('--font-mono', fontFamily);
}

/**
 * Removes the font CSS variables from the root element.
 */
function removeFontVars(root: HTMLElement) {
    root.style.removeProperty('--font-sans');
    root.style.removeProperty('--font-serif');
    root.style.removeProperty('--font-mono');
}

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

    // Initialize config from localStorage
    useEffect(() => {
        const saved = localStorage.getItem('sona-config');
        if (saved) {
            try {
                const parsed = JSON.parse(saved);
                // Check if valid config object
                if (parsed.streamingModelPath || parsed.offlineModelPath || parsed.modelPath || parsed.appLanguage) {

                    // Legacy support for 'modelPath'
                    const legacyPath = parsed.modelPath || '';

                    setConfig({
                        streamingModelPath: parsed.streamingModelPath || legacyPath,
                        offlineModelPath: parsed.offlineModelPath || '',
                        punctuationModelPath: parsed.punctuationModelPath || '',
                        vadModelPath: parsed.vadModelPath || '',
                        enabledITNModels: parsed.enabledITNModels || (parsed.enableITN ? ['itn-zh-number'] : []),
                        itnRulesOrder: parsed.itnRulesOrder || ['itn-zh-number'],
                        vadBufferSize: parsed.vadBufferSize || 5,
                        appLanguage: parsed.appLanguage || 'auto',
                        theme: parsed.theme || 'auto',
                        font: parsed.font || 'system'
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
    }, [setConfig]);

    // Apply theme
    useEffect(() => {
        const theme = config.theme || 'auto';
        const root = document.documentElement;

        if (theme === 'dark') {
            root.setAttribute('data-theme', 'dark');
        } else if (theme === 'light') {
            root.setAttribute('data-theme', 'light');
        } else {
            root.removeAttribute('data-theme');
        }
    }, [config.theme]);

    // Apply font
    useEffect(() => {
        const font = config.font || 'system';
        const root = document.documentElement;

        switch (font) {
            case 'serif':
                setFontVars(root, 'Merriweather, serif');
                break;
            case 'sans':
                setFontVars(root, 'Inter, sans-serif');
                break;
            case 'mono':
                setFontVars(root, 'JetBrains Mono, monospace');
                break;
            case 'arial':
                setFontVars(root, 'Arial, sans-serif');
                break;
            case 'georgia':
                setFontVars(root, 'Georgia, serif');
                break;
            case 'system':
            default:
                removeFontVars(root);
                break;
        }
    }, [config.font]);
}
