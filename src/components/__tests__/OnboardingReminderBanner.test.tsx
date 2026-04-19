import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { OnboardingReminderBanner } from '../OnboardingReminderBanner';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useOnboardingStore } from '../../stores/onboardingStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../services/storageService', () => ({
  settingsStore: {
    set: vi.fn(),
    save: vi.fn(),
    get: vi.fn(),
  },
  STORE_KEY_ONBOARDING: 'sona_onboarding',
}));

describe('OnboardingReminderBanner', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        streamingModelPath: '',
        offlineModelPath: '',
      },
    });
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'deferred' },
      currentStep: 'models',
      entryContext: 'startup',
      isOpen: false,
      focusStartRecordingToken: 0,
    });
  });

  it('renders when models are missing and reopens onboarding on click', () => {
    render(<OnboardingReminderBanner />);

    expect(screen.getByText('first_run.banner.title')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'first_run.banner.cta' }));

    expect(useOnboardingStore.getState().isOpen).toBe(true);
    expect(useOnboardingStore.getState().currentStep).toBe('models');
  });

  it('keeps the banner visible when dismiss confirmation is cancelled', async () => {
    const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(false);

    render(<OnboardingReminderBanner />);

    fireEvent.click(screen.getByRole('button', { name: 'first_run.banner.dismiss_aria_label' }));

    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalled();
    });

    expect(screen.getByText('first_run.banner.title')).toBeDefined();
    expect(useOnboardingStore.getState().persistedState.reminderDismissedAt).toBeUndefined();
  });

  it('dismisses the banner permanently after confirmation', async () => {
    const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(true);

    render(<OnboardingReminderBanner />);

    fireEvent.click(screen.getByRole('button', { name: 'first_run.banner.dismiss_aria_label' }));

    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalled();
      expect(useOnboardingStore.getState().persistedState.reminderDismissedAt).toBeDefined();
    });

    expect(screen.queryByText('first_run.banner.title')).toBeNull();
  });

  it('hides after models are configured manually', () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        streamingModelPath: '/models/live',
        offlineModelPath: '/models/offline',
      },
    });

    render(<OnboardingReminderBanner />);

    expect(screen.queryByText('first_run.banner.title')).toBeNull();
  });

  it('stays hidden when the reminder was dismissed earlier', () => {
    useOnboardingStore.setState({
      persistedState: {
        version: 1,
        status: 'deferred',
        reminderDismissedAt: '2026-03-27T00:00:00.000Z',
      },
    });

    render(<OnboardingReminderBanner />);

    expect(screen.queryByText('first_run.banner.title')).toBeNull();
  });
});
