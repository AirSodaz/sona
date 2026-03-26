import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { OnboardingReminderBanner } from '../OnboardingReminderBanner';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useOnboardingStore } from '../../stores/onboardingStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('OnboardingReminderBanner', () => {
  beforeEach(() => {
    localStorage.clear();
    useTranscriptStore.setState({
      config: {
        ...useTranscriptStore.getState().config,
        streamingModelPath: '',
        offlineModelPath: '',
      },
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

  it('hides after models are configured manually', () => {
    useTranscriptStore.setState({
      config: {
        ...useTranscriptStore.getState().config,
        streamingModelPath: '/models/live',
        offlineModelPath: '/models/offline',
      },
    });

    render(<OnboardingReminderBanner />);

    expect(screen.queryByText('first_run.banner.title')).toBeNull();
  });
});
