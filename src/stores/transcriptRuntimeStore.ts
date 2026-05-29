import { useTranscriptStore, type TranscriptStore } from './transcriptStore';

export const useTranscriptRuntimeStore = Object.assign(
  <T>(selector: (state: TranscriptStore) => T) => {
    return useTranscriptStore(selector);
  },
  {
    getState: () => useTranscriptStore.getState(),
    setState: useTranscriptStore.setState,
    subscribe: useTranscriptStore.subscribe,
  }
);
