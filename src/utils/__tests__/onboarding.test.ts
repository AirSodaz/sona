import { describe, expect, it } from 'vitest';
import {
  getResumeOnboardingStep,
  migrateOnboardingState,
  shouldShowOnboardingReminder,
} from '../onboarding';

describe('onboarding utils', () => {
  it('returns pending for a brand new user', () => {
    expect(migrateOnboardingState(null, null, null)).toEqual({
      version: 1,
      status: 'pending',
    });
  });

  it('treats an existing configured user as completed', () => {
    expect(
      migrateOnboardingState(
        null,
        JSON.stringify({
          streamingModelPath: '/models/live',
          offlineModelPath: '/models/offline',
        }),
        null,
      ),
    ).toEqual({
      version: 1,
      status: 'completed',
    });
  });

  it('maps the legacy completed flag without models to deferred', () => {
    expect(migrateOnboardingState(null, null, 'true')).toEqual({
      version: 1,
      status: 'deferred',
    });
  });

  it('falls back cleanly when onboarding storage is corrupt', () => {
    expect(
      migrateOnboardingState(
        '{broken-json',
        JSON.stringify({
          streamingModelPath: '/models/live',
          offlineModelPath: '/models/offline',
        }),
        null,
      ),
    ).toEqual({
      version: 1,
      status: 'completed',
    });
  });

  it('shows the reminder only when required models are still missing', () => {
    expect(shouldShowOnboardingReminder({ streamingModelPath: '', offlineModelPath: '' })).toBe(true);
    expect(
      shouldShowOnboardingReminder({
        streamingModelPath: '/models/live',
        offlineModelPath: '/models/offline',
      }),
    ).toBe(false);
  });

  it('resumes at the microphone step once models are configured', () => {
    expect(
      getResumeOnboardingStep(
        { streamingModelPath: '/models/live', offlineModelPath: '/models/offline' },
        'startup',
        { version: 1, status: 'deferred' },
      ),
    ).toBe('microphone');
  });
});

