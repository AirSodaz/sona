import { type TranscriptStore, useTranscriptStore } from './transcriptStore';

export const transcriptSidecarStore = {
  getState: () => useTranscriptStore.getState(),
  setState: useTranscriptStore.setState,
  subscribe: useTranscriptStore.subscribe,
};

export const useTranscriptSidecarStore = Object.assign(
  <T>(selector: (state: TranscriptStore) => T) => {
    return useTranscriptStore(selector);
  },
  transcriptSidecarStore
);

export type { AutoSaveState, AutoSaveStatus, LlmState } from './transcriptSidecarState';
