import { beforeEach, describe, expect, it } from 'vitest';
import { useOnboardingStore } from '../onboardingStore';
import { ONBOARDING_STORAGE_KEY } from '../../utils/onboarding';

describe('onboardingStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'deferred' },
      currentStep: 'models',
      entryContext: 'startup',
      isOpen: false,
      focusStartRecordingToken: 0,
    });
  });

  it('dismissReminder preserves onboarding status and writes reminderDismissedAt', () => {
    useOnboardingStore.getState().dismissReminder();

    const persistedState = useOnboardingStore.getState().persistedState;
    const storedState = JSON.parse(localStorage.getItem(ONBOARDING_STORAGE_KEY) || '{}');

    expect(persistedState.status).toBe('deferred');
    expect(persistedState.reminderDismissedAt).toBeDefined();
    expect(storedState.status).toBe('deferred');
    expect(storedState.reminderDismissedAt).toBe(persistedState.reminderDismissedAt);
  });
});
