import React from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { getResumeOnboardingStep, shouldShowOnboardingReminder } from '../utils/onboarding';

/**
 * Inline reminder that reopens onboarding when required setup is still missing.
 */
export function OnboardingReminderBanner(): React.JSX.Element | null {
  const { t } = useTranslation();
  const config = useTranscriptStore((state) => state.config);
  const { isOpen, persistedState, reopen } = useOnboardingStore((state) => ({
    isOpen: state.isOpen,
    persistedState: state.persistedState,
    reopen: state.reopen,
  }));

  if (isOpen || !shouldShowOnboardingReminder(config)) {
    return null;
  }

  return (
    <div className="onboarding-reminder-banner" role="status" aria-live="polite">
      <div className="onboarding-reminder-copy">
        <strong>{t('first_run.banner.title')}</strong>
        <span>{t('first_run.banner.body')}</span>
      </div>
      <button
        className="btn btn-secondary"
        onClick={() => reopen(getResumeOnboardingStep(config, 'startup', persistedState), 'startup')}
      >
        {t('first_run.banner.cta')}
      </button>
    </div>
  );
}
