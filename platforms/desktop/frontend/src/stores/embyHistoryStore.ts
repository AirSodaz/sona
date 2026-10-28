import { create } from 'zustand';
import type { EmbyTaskContext } from '../types/batchQueue';

const STORAGE_KEY = 'sona:emby:history_contexts';

function loadStoredContexts(): Record<string, EmbyTaskContext> {
  try {
    if (typeof localStorage === 'undefined') return {};
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

function saveStoredContexts(contexts: Record<string, EmbyTaskContext>): void {
  try {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(contexts));
  } catch {
    // Ignore storage quota errors
  }
}

interface EmbyHistoryStore {
  contexts: Record<string, EmbyTaskContext>;
  setContext: (historyId: string, context: EmbyTaskContext) => void;
  getContext: (historyId: string) => EmbyTaskContext | undefined;
  removeContext: (historyId: string) => void;
}

export const useEmbyHistoryStore = create<EmbyHistoryStore>((set, get) => ({
  contexts: loadStoredContexts(),

  setContext: (historyId, context) => {
    set((state) => {
      const next = { ...state.contexts, [historyId]: context };
      saveStoredContexts(next);
      return { contexts: next };
    });
  },

  getContext: (historyId) => get().contexts[historyId],

  removeContext: (historyId) => {
    set((state) => {
      const next = { ...state.contexts };
      delete next[historyId];
      saveStoredContexts(next);
      return { contexts: next };
    });
  },
}));
