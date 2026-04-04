import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { CloseIcon } from './Icons';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useShallow } from 'zustand/react/shallow';
import { getResumeOnboardingStep, shouldShowOnboardingReminder } from '../utils/onboarding';

/**
 * Inline reminder that reopens onboarding when required setup is still missing.
 */
export function OnboardingReminderBanner(): React.JSX.Element | null {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const confirm = useDialogStore((state) => state.confirm);
  const { isOpen, persistedState, dismissReminder, reopen } = useOnboardingStore(
    useShallow((state) => ({
      isOpen: state.isOpen,
      persistedState: state.persistedState,
      dismissReminder: state.dismissReminder,
      reopen: state.reopen,
    }))
  );

  const handleDismiss = useCallback(async () => {
    const confirmed = await confirm(t('first_run.banner.dismiss_confirm_message'), {
      title: t('first_run.banner.dismiss_confirm_title'),
      variant: 'warning',
      confirmLabel: t('first_run.banner.dismiss_confirm_action'),
    });
    if (!confirmed) {
      return;
    }

    dismissReminder();
  }, [confirm, dismissReminder, t]);

  if (isOpen || !shouldShowOnboardingReminder(config, persistedState)) {
    return null;
  }

  return (
    <div className="onboarding-reminder-banner" role="status" aria-live="polite">
      <div className="onboarding-reminder-copy">
        <strong>{t('first_run.banner.title')}</strong>
        <span>{t('first_run.banner.body')}</span>
      </div>
      <div className="onboarding-reminder-actions">
        <button
          className="btn btn-secondary"
          onClick={() => reopen(getResumeOnboardingStep(config, 'startup', persistedState), 'startup')}
        >
          {t('first_run.banner.cta')}
        </button>
        <button
          className="btn btn-icon onboarding-reminder-dismiss"
          aria-label={t('first_run.banner.dismiss_aria_label')}
          onClick={() => {
            void handleDismiss();
          }}
        >
          <CloseIcon />
        </button>
      </div>
    </div>
  );
}
