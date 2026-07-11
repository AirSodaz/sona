import { useTranscriptStore, type TranscriptStore } from './transcriptStore';
import type { LexicalEditor } from 'lexical';

let activeEditor: LexicalEditor | null = null;

/** Stores the active LexicalEditor instance for toolbar ↔ editor communication. */
export function setActiveEditor(editor: LexicalEditor | null): void {
  activeEditor = editor;
}

/** Returns the currently focused LexicalEditor instance, if any. */
export function getActiveEditor(): LexicalEditor | null {
  return activeEditor;
}

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
