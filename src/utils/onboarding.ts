import { AppConfig } from '../types/transcript';
import {
  OnboardingEntryContext,
  OnboardingState,
  OnboardingStatus,
  OnboardingStep,
} from '../types/onboarding';

export const ONBOARDING_STORAGE_KEY = 'sona-onboarding';
export const LEGACY_FIRST_RUN_KEY = 'sona-first-run-completed';
export const ONBOARDING_VERSION = 1;
export type {
  OnboardingEntryContext,
  OnboardingState,
  OnboardingStatus,
  OnboardingStep,
} from '../types/onboarding';

type LegacyConfig = Partial<AppConfig> & {
  recognitionModelPath?: string;
  modelPath?: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isValidStatus(value: unknown): value is OnboardingStatus {
  return value === 'pending' || value === 'deferred' || value === 'completed';
}

function createState(
  status: OnboardingStatus,
  overrides: Partial<Pick<OnboardingState, 'deferredAt' | 'completedAt' | 'reminderDismissedAt'>> = {},
): OnboardingState {
  return {
    version: ONBOARDING_VERSION,
    status,
    ...overrides,
  };
}

/**
 * Parses the persisted app config enough to reason about onboarding migration.
 */
export function parseStoredConfig(rawValue: string | null): Partial<AppConfig> {
  if (!rawValue) {
    return {};
  }

  try {
    const parsed = JSON.parse(rawValue);
    if (!isRecord(parsed)) {
      return {};
    }

    const config = parsed as LegacyConfig;

    return {
      streamingModelPath:
        config.streamingModelPath ||
        config.recognitionModelPath ||
        config.offlineModelPath ||
        config.modelPath ||
        '',
      offlineModelPath:
        config.offlineModelPath ||
        config.recognitionModelPath ||
        config.modelPath ||
        '',
      vadModelPath: config.vadModelPath || '',
      microphoneId: config.microphoneId || 'default',
    };
  } catch (error) {
    console.error('[Onboarding] Failed to parse stored config:', error);
    return {};
  }
}

/**
 * Checks whether any recognition model has already been configured.
 */
export function hasConfiguredRecognitionModel(config?: Partial<AppConfig> | null): boolean {
  if (!config) {
    return false;
  }

  return Boolean(config.streamingModelPath || config.offlineModelPath);
}

/**
 * Checks whether the minimum recommended onboarding models are configured.
 */
export function hasRequiredOnboardingModels(config?: Partial<AppConfig> | null): boolean {
  if (!config) {
    return false;
  }

  return Boolean(config.streamingModelPath && config.offlineModelPath);
}

/**
 * Determines whether the reminder banner should remain visible.
 */
export function shouldShowOnboardingReminder(
  config?: Partial<AppConfig> | null,
  state?: OnboardingState | null,
): boolean {
  return !hasRequiredOnboardingModels(config) && !state?.reminderDismissedAt;
}

/**
 * Chooses the best onboarding step to resume from for the given state.
 */
export function getResumeOnboardingStep(
  config?: Partial<AppConfig> | null,
  entryContext: OnboardingEntryContext = 'startup',
  state?: OnboardingState | null,
): OnboardingStep {
  if (hasRequiredOnboardingModels(config)) {
    return 'microphone';
  }

  if (entryContext === 'startup' && state?.status === 'pending') {
    return 'welcome';
  }

  return 'models';
}

/**
 * Migrates legacy first-run flags into the new onboarding state model.
 */
export function migrateOnboardingState(
  storedOnboardingValue: string | null,
  storedConfigValue: string | null,
  legacyFirstRunCompleted: string | null,
): OnboardingState {
  if (storedOnboardingValue) {
    try {
      const parsed = JSON.parse(storedOnboardingValue);
      if (isRecord(parsed) && isValidStatus(parsed.status)) {
        return createState(parsed.status, {
          deferredAt: typeof parsed.deferredAt === 'string' ? parsed.deferredAt : undefined,
          completedAt: typeof parsed.completedAt === 'string' ? parsed.completedAt : undefined,
          reminderDismissedAt:
            typeof parsed.reminderDismissedAt === 'string'
              ? parsed.reminderDismissedAt
              : undefined,
        });
      }
    } catch (error) {
      console.error('[Onboarding] Failed to parse onboarding state:', error);
    }
  }

  const storedConfig = parseStoredConfig(storedConfigValue);
  if (hasConfiguredRecognitionModel(storedConfig)) {
    return createState('completed');
  }

  if (legacyFirstRunCompleted === 'true') {
    return createState('deferred');
  }

  return createState('pending');
}

/**
 * Reads onboarding state from browser storage with legacy migration support.
 */
export function readOnboardingState(storage?: Pick<Storage, 'getItem'> | null): OnboardingState {
  if (!storage) {
    return createState('pending');
  }

  return migrateOnboardingState(
    storage.getItem(ONBOARDING_STORAGE_KEY),
    storage.getItem('sona-config'),
    storage.getItem(LEGACY_FIRST_RUN_KEY),
  );
}

/**
 * Persists onboarding state and removes the legacy first-run key.
 */
export function writeOnboardingState(
  state: OnboardingState,
  storage?: Pick<Storage, 'setItem' | 'removeItem'> | null,
): void {
  if (!storage) {
    return;
  }

  storage.setItem(ONBOARDING_STORAGE_KEY, JSON.stringify(state));
  storage.removeItem(LEGACY_FIRST_RUN_KEY);
}
