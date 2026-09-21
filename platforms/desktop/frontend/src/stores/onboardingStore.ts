import { create } from 'zustand';
import { STORE_KEY_ONBOARDING, settingsStore } from '../services/storageService';
import type { OnboardingEntryContext, OnboardingState, OnboardingStep } from '../types/onboarding';
import { getResumeOnboardingStep } from '../utils/onboarding';

interface OnboardingStoreState {
  persistedState: OnboardingState;
  currentStep: OnboardingStep;
  entryContext: OnboardingEntryContext;
  isOpen: boolean;
  focusStartRecordingToken: number;
  modelDownloadStatus: 'idle' | 'downloading' | 'completed' | 'failed';
  modelDownloadProgress: number;
  modelDownloadError: string;
  open: (step?: OnboardingStep, context?: OnboardingEntryContext) => void;
  close: () => void;
  setStep: (step: OnboardingStep) => void;
  defer: () => void;
  complete: () => void;
  dismissReminder: () => void;
  reopen: (step?: OnboardingStep, context?: OnboardingEntryContext) => void;
  setPersistedState: (state: OnboardingState, configHasModels: boolean) => void;
  setModelDownloadStatus: (
    status: 'idle' | 'downloading' | 'completed' | 'failed',
    progress?: number,
    error?: string
  ) => void;
}

const defaultState: OnboardingState = { version: 1, status: 'pending' };

/** Shared store for onboarding visibility, progress, and completion state. */
export const useOnboardingStore = create<OnboardingStoreState>((set, get) => ({
  persistedState: defaultState,
  currentStep: 'models',
  entryContext: 'startup',
  isOpen: false,
  focusStartRecordingToken: 0,
  modelDownloadStatus: 'idle' as const,
  modelDownloadProgress: 0,
  modelDownloadError: '',

  setPersistedState: (state: OnboardingState, configHasModels: boolean) => {
    // We mock a partial config just for `getResumeOnboardingStep` to know if models exist.
    const mockConfig = configHasModels
      ? { streamingModelPath: 'mock', batchModelPath: 'mock' }
      : undefined;
    set({
      persistedState: state,
      currentStep: getResumeOnboardingStep(mockConfig, 'startup', state),
      isOpen: state.status === 'pending',
    });
  },

  open: (step = 'models', context = 'startup') =>
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

  defer: async () => {
    const previousState = get().persistedState;
    const nextState: OnboardingState = {
      version: 1,
      status: 'deferred',
      deferredAt: new Date().toISOString(),
      reminderDismissedAt: previousState.reminderDismissedAt,
    };

    set({
      persistedState: nextState,
      isOpen: false,
    });

    await settingsStore.set(STORE_KEY_ONBOARDING, nextState);
    await settingsStore.save();
  },

  complete: async () => {
    const previousState = get().persistedState;
    const nextState: OnboardingState = {
      version: 1,
      status: 'completed',
      completedAt: new Date().toISOString(),
      reminderDismissedAt: previousState.reminderDismissedAt,
    };

    set((state) => ({
      persistedState: nextState,
      isOpen: false,
      entryContext: 'startup',
      focusStartRecordingToken: state.focusStartRecordingToken + 1,
      modelDownloadStatus: 'idle' as const,
      modelDownloadProgress: 0,
      modelDownloadError: '',
    }));

    await settingsStore.set(STORE_KEY_ONBOARDING, nextState);
    await settingsStore.save();
  },

  dismissReminder: async () => {
    const previousState = get().persistedState;
    const nextState: OnboardingState = {
      ...previousState,
      reminderDismissedAt: previousState.reminderDismissedAt || new Date().toISOString(),
    };

    set({
      persistedState: nextState,
    });

    await settingsStore.set(STORE_KEY_ONBOARDING, nextState);
    await settingsStore.save();
  },

  reopen: (step = get().currentStep, context = 'startup') => {
    get().open(step, context);
  },

  setModelDownloadStatus: (status, progress?, error?) => {
    set({
      modelDownloadStatus: status,
      modelDownloadProgress: progress ?? get().modelDownloadProgress,
      modelDownloadError: error ?? '',
    });
  },
}));
