import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useOnboardingStore } from '../onboardingStore';

// Mock storage service
vi.mock('../../services/storageService', () => ({
  settingsStore: {
    set: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined),
    get: vi.fn().mockResolvedValue(null),
  },
  STORE_KEY_ONBOARDING: 'sona-onboarding',
}));

describe('onboardingStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'deferred' },
      currentStep: 'models',
      entryContext: 'startup',
      isOpen: false,
      focusStartRecordingToken: 0,
    });
  });

  it('dismissReminder preserves onboarding status and writes reminderDismissedAt', async () => {
    await useOnboardingStore.getState().dismissReminder();

    const persistedState = useOnboardingStore.getState().persistedState;

    expect(persistedState.status).toBe('deferred');
    expect(persistedState.reminderDismissedAt).toBeDefined();

    // Verify it was saved to storage
    const { settingsStore, STORE_KEY_ONBOARDING } = await import('../../services/storageService');
    expect(settingsStore.set).toHaveBeenCalledWith(STORE_KEY_ONBOARDING, expect.objectContaining({
      status: 'deferred',
      reminderDismissedAt: persistedState.reminderDismissedAt
    }));
  });
});
