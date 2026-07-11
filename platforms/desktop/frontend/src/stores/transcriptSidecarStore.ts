import { useTranscriptStore, type TranscriptStore } from './transcriptStore';

export const useTranscriptSidecarStore = Object.assign(
  <T>(selector: (state: TranscriptStore) => T) => {
    return useTranscriptStore(selector);
  },
  {
    getState: () => useTranscriptStore.getState(),
    setState: useTranscriptStore.setState,
    subscribe: useTranscriptStore.subscribe,
  }
);

export type { AutoSaveState, AutoSaveStatus, LlmState } from './transcriptSidecarState';
