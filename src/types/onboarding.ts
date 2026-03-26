/** Persisted onboarding completion states. */
export type OnboardingStatus = 'pending' | 'deferred' | 'completed';
/** Individual onboarding wizard steps. */
export type OnboardingStep = 'welcome' | 'models' | 'microphone';
/** Entry points that can reopen onboarding. */
export type OnboardingEntryContext = 'startup' | 'live_record' | 'batch_import';

/** Persisted first-run onboarding state. */
export interface OnboardingState {
  version: 1;
  status: OnboardingStatus;
  deferredAt?: string;
  completedAt?: string;
  reminderDismissedAt?: string;
}
