import type { LexicalEditor } from 'lexical';
import { type TranscriptStore, useTranscriptStore } from './transcriptStore';

/** Stores the active LexicalEditor instance for toolbar ↔ editor communication. */
export function setActiveEditor(editor: LexicalEditor | null): void {
  useTranscriptStore.getState().setActiveEditor(editor);
}

/** Returns the currently focused LexicalEditor instance, if any. */
export function getActiveEditor(): LexicalEditor | null {
  return useTranscriptStore.getState().activeEditor;
}

export const transcriptRuntimeStore = {
  getState: () => useTranscriptStore.getState(),
  setState: useTranscriptStore.setState,
  subscribe: useTranscriptStore.subscribe,
};

export const useTranscriptRuntimeStore = Object.assign(
  <T>(selector: (state: TranscriptStore) => T) => {
    return useTranscriptStore(selector);
  },
  transcriptRuntimeStore
);
