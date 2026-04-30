import i18n from '../../i18n';
import { useConfigStore } from '../../stores/configStore';
import { useOnboardingStore } from '../../stores/onboardingStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import type { AppConfig } from '../../types/config';
import { migrateProjectPolishDefaults } from '../../types/project';
import type { OnboardingState } from '../../types/onboarding';
import { logger } from '../../utils/logger';
import {
  LEGACY_FIRST_RUN_KEY,
  migrateOnboardingState,
  ONBOARDING_STORAGE_KEY,
} from '../../utils/onboarding';
import { migrateConfig } from '../configMigrationService';
import { projectService } from '../projectService';
import { settingsStore, STORE_KEY_CONFIG, STORE_KEY_ONBOARDING } from '../storageService';

const LEGACY_CONFIG_STORAGE_KEY = STORE_KEY_CONFIG;

interface HydratedOnboardingResult {
  state: OnboardingState;
  migrated: boolean;
  clearLegacyOnboarding: boolean;
  clearLegacyFirstRun: boolean;
}

interface StartupPersistencePlan {
  configToPersist: AppConfig | null;
  onboardingToPersist: OnboardingState | null;
  configLegacyKeyToClear: boolean;
  onboardingLegacyKeyToClear: boolean;
  firstRunLegacyKeyToClear: boolean;
}

function parseLegacyConfigValue(rawValue: string | null): Record<string, unknown> | null {
  if (!rawValue) {
    return null;
  }

  try {
    const parsed = JSON.parse(rawValue);
    return typeof parsed === 'object' && parsed !== null
      ? parsed as Record<string, unknown>
      : null;
  } catch (error) {
    logger.error('[Startup] Failed to parse legacy config:', error);
    return null;
  }
}

function applyHydratedConfig(config: AppConfig): void {
  useConfigStore.getState().setConfig(config);

  if (config.startOnLaunch) {
    useTranscriptRuntimeStore.getState().setIsCaptionMode(true);
  }

  if (config.appLanguage && config.appLanguage !== 'auto') {
    void i18n.changeLanguage(config.appLanguage);
    return;
  }

  void i18n.changeLanguage(navigator.language);
}

async function hydrateOnboardingState(
  config: AppConfig,
  configMigrated: boolean,
): Promise<HydratedOnboardingResult> {
  const savedOnboarding = await settingsStore.get<OnboardingState | null>(STORE_KEY_ONBOARDING);

  if (savedOnboarding) {
    return {
      state: savedOnboarding,
      migrated: false,
      clearLegacyOnboarding: false,
      clearLegacyFirstRun: false,
    };
  }

  const legacyOnboardingValue = localStorage.getItem(ONBOARDING_STORAGE_KEY);
  const legacyFirstRunValue = localStorage.getItem(LEGACY_FIRST_RUN_KEY);

  if (legacyOnboardingValue || legacyFirstRunValue || configMigrated) {
    return {
      state: migrateOnboardingState(
        legacyOnboardingValue,
        JSON.stringify(config),
        legacyFirstRunValue,
      ),
      migrated: true,
      clearLegacyOnboarding: legacyOnboardingValue !== null,
      clearLegacyFirstRun: legacyFirstRunValue !== null,
    };
  }

  return {
    state: { version: 1, status: 'pending' },
    migrated: false,
    clearLegacyOnboarding: false,
    clearLegacyFirstRun: false,
  };
}

async function persistHydratedState(plan: StartupPersistencePlan): Promise<void> {
  let didPersistStoreWrites = false;

  try {
    if (plan.configToPersist) {
      await settingsStore.set(STORE_KEY_CONFIG, plan.configToPersist);
      didPersistStoreWrites = true;
    }

    if (plan.onboardingToPersist) {
      await settingsStore.set(STORE_KEY_ONBOARDING, plan.onboardingToPersist);
      didPersistStoreWrites = true;
    }

    if (didPersistStoreWrites) {
      await settingsStore.save();
    }
  } catch (error) {
    logger.error('[Startup] Failed to persist hydrated startup state:', error);
    return;
  }

  if (plan.configLegacyKeyToClear && plan.configToPersist) {
    localStorage.removeItem(LEGACY_CONFIG_STORAGE_KEY);
  }

  if (plan.onboardingLegacyKeyToClear && plan.onboardingToPersist) {
    localStorage.removeItem(ONBOARDING_STORAGE_KEY);
  }

  if (plan.firstRunLegacyKeyToClear && plan.onboardingToPersist) {
    localStorage.removeItem(LEGACY_FIRST_RUN_KEY);
  }
}

async function maybeMigrateProjectPolishDefaults(config: AppConfig): Promise<{
  config: AppConfig;
  configChanged: boolean;
}> {
  const migration = migrateProjectPolishDefaults(
    useProjectStore.getState().projects,
    config.polishCustomPresets,
  );

  if (!migration.migrated) {
    return {
      config,
      configChanged: false,
    };
  }

  try {
    await projectService.saveAll(migration.projects);
    useProjectStore.setState({ projects: migration.projects });

    const nextConfig: AppConfig = {
      ...config,
      polishCustomPresets: migration.customPresets,
    };
    useConfigStore.getState().setConfig({ polishCustomPresets: migration.customPresets });

    return {
      config: nextConfig,
      configChanged: true,
    };
  } catch (error) {
    logger.error('[Startup] Failed to persist migrated project polish defaults:', error);
    return {
      config,
      configChanged: false,
    };
  }
}

export async function hydrateAppStartupState(): Promise<void> {
  try {
    const savedConfig = await settingsStore.get<AppConfig | null>(STORE_KEY_CONFIG);
    const legacyConfigValue = savedConfig ? null : localStorage.getItem(LEGACY_CONFIG_STORAGE_KEY);
    const legacyConfig = savedConfig ? null : parseLegacyConfigValue(legacyConfigValue);

    const { config: loadedConfig, migrated: configMigrated } = await migrateConfig(savedConfig, legacyConfig);
    applyHydratedConfig(loadedConfig);

    const onboarding = await hydrateOnboardingState(loadedConfig, configMigrated);
    useOnboardingStore.getState().setPersistedState(
      onboarding.state,
      Boolean(loadedConfig.streamingModelPath && loadedConfig.offlineModelPath),
    );

    await useProjectStore.getState().loadProjects();

    const projectMigration = await maybeMigrateProjectPolishDefaults(loadedConfig);
    const configToPersist = projectMigration.config;

    await persistHydratedState({
      configToPersist: (configMigrated || projectMigration.configChanged) ? configToPersist : null,
      onboardingToPersist: onboarding.migrated ? onboarding.state : null,
      configLegacyKeyToClear: Boolean(legacyConfigValue),
      onboardingLegacyKeyToClear: onboarding.clearLegacyOnboarding,
      firstRunLegacyKeyToClear: onboarding.clearLegacyFirstRun,
    });
  } catch (error) {
    logger.error('[Startup] Failed to hydrate app startup state:', error);
  }
}
