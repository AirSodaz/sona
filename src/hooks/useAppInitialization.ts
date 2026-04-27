import { useEffect, useState } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useProjectStore } from '../stores/projectStore';
import { useAutomationStore } from '../stores/automationStore';
import i18n from '../i18n';
import { migrateConfig } from '../services/configMigrationService';
import { settingsStore, STORE_KEY_CONFIG, STORE_KEY_ONBOARDING } from '../services/storageService';
import { projectService } from '../services/projectService';
import { migrateOnboardingState, LEGACY_FIRST_RUN_KEY, ONBOARDING_STORAGE_KEY } from '../utils/onboarding';
import { AppConfig } from '../types/config';
import { migrateProjectPolishDefaults } from '../types/project';
import { logger } from '../utils/logger';
import { useThemeEffect } from './useThemeEffect';
import { useFontEffect } from './useFontEffect';
import { useConfigPersistence } from './useConfigPersistence';
import { useTraySyncEffect } from './useTraySyncEffect';
import { voiceTypingService } from '../services/voiceTypingService';
import { healthCheckService } from '../services/healthCheckService';

/**
 * Hook to handle application initialization.
 *
 * - Loads configuration from Tauri store (or migrates from localStorage).
 * - Loads onboarding state from Tauri store (or migrates from localStorage).
 * - Initializes decoupled side-effects (theme, font, persistence, tray).
 */
export function useAppInitialization() {
    const setConfig = useConfigStore((state) => state.setConfig);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const setPersistedState = useOnboardingStore((state) => state.setPersistedState);
    const loadProjects = useProjectStore((state) => state.loadProjects);
    const loadAutomation = useAutomationStore((state: any) => state.loadAndStart);
    const [isLoaded, setIsLoaded] = useState(false);

    // Initialize decoupled side-effects
    useThemeEffect();
    useFontEffect();
    useConfigPersistence(isLoaded);
    useTraySyncEffect(isLoaded);

    // Initialize config and onboarding state
    useEffect(() => {
        async function initialize() {
            try {
                let hasPendingSettingsSave = false;

                // 1. Load config
                let savedConfig = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
                const { config: loadedConfig, migrated: isConfigMigrated } = await migrateConfig(savedConfig);

                setConfig(loadedConfig);
                hasPendingSettingsSave = isConfigMigrated;

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

                hasPendingSettingsSave = hasPendingSettingsSave || isOnboardingMigrated;

                await loadProjects();

                const projectPresetMigration = migrateProjectPolishDefaults(
                    useProjectStore.getState().projects,
                    useConfigStore.getState().config.polishCustomPresets,
                );

                if (projectPresetMigration.migrated) {
                    useProjectStore.setState({ projects: projectPresetMigration.projects });
                    setConfig({ polishCustomPresets: projectPresetMigration.customPresets });
                    await projectService.saveAll(projectPresetMigration.projects);
                    await settingsStore.set(STORE_KEY_CONFIG, {
                        ...useConfigStore.getState().config,
                        polishCustomPresets: projectPresetMigration.customPresets,
                    });
                    hasPendingSettingsSave = true;
                }

                if (hasPendingSettingsSave) {
                    await settingsStore.save();
                }

                await loadAutomation();

                // Initialize Voice Typing shortcut listeners (Main Window Only)
                voiceTypingService.init();

                // Run background health check to ensure data consistency
                healthCheckService.runHealthCheck();

            } catch (e) {
                logger.error('Failed to initialize app state:', e);
            } finally {
                setIsLoaded(true);
            }
        }

        initialize();
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [loadAutomation, loadProjects]); // Run once on mount

    return { isLoaded };
}
