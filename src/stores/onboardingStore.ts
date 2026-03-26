import { create } from 'zustand';
import {
  OnboardingEntryContext,
  OnboardingState,
  OnboardingStep,
} from '../types/onboarding';
import {
  getResumeOnboardingStep,
  readOnboardingState,
  writeOnboardingState,
} from '../utils/onboarding';

interface OnboardingStoreState {
  persistedState: OnboardingState;
  currentStep: OnboardingStep;
  entryContext: OnboardingEntryContext;
  isOpen: boolean;
  focusStartRecordingToken: number;
  open: (step?: OnboardingStep, context?: OnboardingEntryContext) => void;
  close: () => void;
  setStep: (step: OnboardingStep) => void;
  defer: () => void;
  complete: () => void;
  reopen: (step?: OnboardingStep, context?: OnboardingEntryContext) => void;
}

const initialPersistedState = readOnboardingState(
  typeof window !== 'undefined' ? window.localStorage : null,
);

/** Shared store for onboarding visibility, progress, and completion state. */
export const useOnboardingStore = create<OnboardingStoreState>((set, get) => ({
  persistedState: initialPersistedState,
  currentStep: getResumeOnboardingStep(undefined, 'startup', initialPersistedState),
  entryContext: 'startup',
  isOpen: initialPersistedState.status === 'pending',
  focusStartRecordingToken: 0,

  open: (step = 'welcome', context = 'startup') =>
    set({
      currentStep: step,
      entryContext: context,
      isOpen: true,
    }),

  close: () =>
    set({
      isOpen: false,
    }),

  setStep: (step) =>
    set({
      currentStep: step,
    }),

  defer: () => {
    const nextState: OnboardingState = {
      version: 1,
      status: 'deferred',
      deferredAt: new Date().toISOString(),
    };

    writeOnboardingState(nextState, typeof window !== 'undefined' ? window.localStorage : null);

    set({
      persistedState: nextState,
      isOpen: false,
    });
  },

  complete: () => {
    const nextState: OnboardingState = {
      version: 1,
      status: 'completed',
      completedAt: new Date().toISOString(),
    };

    writeOnboardingState(nextState, typeof window !== 'undefined' ? window.localStorage : null);

    set((state) => ({
      persistedState: nextState,
      isOpen: false,
      entryContext: 'startup',
      focusStartRecordingToken: state.focusStartRecordingToken + 1,
    }));
  },

  reopen: (step = get().currentStep, context = 'startup') => {
    get().open(step, context);
  },
}));
