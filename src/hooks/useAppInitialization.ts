import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore, useUIConfig } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import i18n from '../i18n';
import { migrateConfig } from '../services/configMigrationService';
import { settingsStore, STORE_KEY_CONFIG, STORE_KEY_ONBOARDING } from '../services/storageService';
import { migrateOnboardingState, LEGACY_FIRST_RUN_KEY, ONBOARDING_STORAGE_KEY } from '../utils/onboarding';
import { AppConfig } from '../types/config';
import { logger } from '../utils/logger';

/**
 * Hook to handle application initialization.
 *
 * - Loads configuration from Tauri store (or migrates from localStorage).
 * - Loads onboarding state from Tauri store (or migrates from localStorage).
 * - Applies theme settings.
 * - Applies font settings.
 * - Persists config changes (debounced).
 */
export function useAppInitialization() {
    const config = useConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const setPersistedState = useOnboardingStore((state) => state.setPersistedState);
    const [isLoaded, setIsLoaded] = useState(false);

    // Domain-specific selectors for fine-grained dependency tracking
    const { theme, font, minimizeToTrayOnExit } = useUIConfig();

    // Initialize config and onboarding state
    useEffect(() => {
        async function initialize() {
            try {
                // 1. Load config
                let savedConfig = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
                const { config: loadedConfig, migrated: isConfigMigrated } = await migrateConfig(savedConfig);

                setConfig(loadedConfig);

                if (loadedConfig.startOnLaunch) {
                    setIsCaptionMode(true);
                }

                if (loadedConfig.appLanguage && loadedConfig.appLanguage !== 'auto') {
                    i18n.changeLanguage(loadedConfig.appLanguage);
                } else {
                    i18n.changeLanguage(navigator.language);
                }

                // 2. Load onboarding state
                let savedOnboarding = await settingsStore.get<any>(STORE_KEY_ONBOARDING);
                let isOnboardingMigrated = false;

                if (!savedOnboarding) {
                    // Try to migrate from localStorage
                    const legacyOnboarding = localStorage.getItem(ONBOARDING_STORAGE_KEY);
                    const legacyFirstRun = localStorage.getItem(LEGACY_FIRST_RUN_KEY);
                    
                    if (legacyOnboarding || legacyFirstRun || isConfigMigrated) {
                        const legacyConfigString = JSON.stringify(loadedConfig);
                        savedOnboarding = migrateOnboardingState(legacyOnboarding, legacyConfigString, legacyFirstRun);
                        isOnboardingMigrated = true;
                    }
                }

                if (savedOnboarding) {
                    setPersistedState(savedOnboarding, !!(loadedConfig.streamingModelPath && loadedConfig.offlineModelPath));
                    
                    if (isOnboardingMigrated) {
                        await settingsStore.set(STORE_KEY_ONBOARDING, savedOnboarding);
                        localStorage.removeItem(ONBOARDING_STORAGE_KEY);
                        localStorage.removeItem(LEGACY_FIRST_RUN_KEY);
                    }
                } else {
                    // Default fallback
                    setPersistedState({ version: 1, status: 'pending' }, false);
                }

                if (isConfigMigrated || isOnboardingMigrated) {
                    await settingsStore.save();
                }
            } catch (e) {
                logger.error('Failed to initialize app state:', e);
            } finally {
                setIsLoaded(true);
            }
        }

        initialize();
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []); // Run once on mount

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

    // Persist config changes to Tauri store (debounced)
    useEffect(() => {
        if (!isLoaded) return;

        const timeoutId = setTimeout(async () => {
            try {
                await settingsStore.set(STORE_KEY_CONFIG, config);
                await settingsStore.save();
            } catch (e) {
                logger.error('Failed to save config to store:', e);
            }
        }, 500);

        return () => clearTimeout(timeoutId);
    }, [config, isLoaded]);

    // Sync minimize to tray setting with backend
    useEffect(() => {
        if (!isLoaded) return;
        invoke('set_minimize_to_tray', { enabled: minimizeToTrayOnExit ?? true })
            .catch(e => logger.error('Failed to set minimize to tray:', e));
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

    return { isLoaded };
}
