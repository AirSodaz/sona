import type { LexicalEditor } from 'lexical';
import { type TranscriptStore, useTranscriptStore } from './transcriptStore';

let activeLexicalEditor: LexicalEditor | null = null;

/** Stores the active LexicalEditor instance for toolbar ↔ editor communication without polluting reactive store state. */
export function setActiveEditor(editor: LexicalEditor | null): void {
  activeLexicalEditor = editor;
}

/** Clears the active LexicalEditor instance, ensuring it is only cleared if the current active editor matches expectedEditor (or if expectedEditor is omitted). */
export function clearActiveEditor(expectedEditor?: LexicalEditor | null): void {
  if (!expectedEditor || activeLexicalEditor === expectedEditor) {
    activeLexicalEditor = null;
  }
}

/** Returns the currently focused LexicalEditor instance, if any. */
export function getActiveEditor(): LexicalEditor | null {
  return activeLexicalEditor;
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
