import { create } from 'zustand';
import type { EmbyTaskContext } from '../types/batchQueue';

const STORAGE_KEY = 'sona:emby:history_contexts';
const MAX_STORED_CONTEXTS = 200;

function trimContexts(contexts: Record<string, EmbyTaskContext>): Record<string, EmbyTaskContext> {
  const keys = Object.keys(contexts);
  if (keys.length <= MAX_STORED_CONTEXTS) {
    return contexts;
  }
  const excess = keys.length - MAX_STORED_CONTEXTS;
  const pruned = { ...contexts };
  for (let i = 0; i < excess; i++) {
    delete pruned[keys[i]];
  }
  return pruned;
}

function loadStoredContexts(): Record<string, EmbyTaskContext> {
  try {
    if (typeof localStorage === 'undefined') return {};
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? trimContexts(parsed)
      : {};
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
    let trimmedResult: Record<string, EmbyTaskContext> = {};
    set((state) => {
      const next = { ...state.contexts };
      delete next[historyId];
      next[historyId] = context;
      trimmedResult = trimContexts(next);
      return { contexts: trimmedResult };
    });
    saveStoredContexts(trimmedResult);
  },

  getContext: (historyId) =>
    Object.hasOwn(get().contexts, historyId) ? get().contexts[historyId] : undefined,

  removeContext: (historyId) => {
    let changed = false;
    let nextContexts: Record<string, EmbyTaskContext> = {};
    set((state) => {
      if (!Object.hasOwn(state.contexts, historyId)) return state;
      changed = true;
      const next = { ...state.contexts };
      delete next[historyId];
      nextContexts = next;
      return { contexts: next };
    });
    if (changed) {
      saveStoredContexts(nextContexts);
    }
  },
}));
