import { create } from 'zustand';
import type { MediaServerTaskContext } from '../types/batchQueue';

const STORAGE_KEY = 'sona:media_server:history_contexts';
const LEGACY_STORAGE_KEY = 'sona:emby:history_contexts';
const MAX_STORED_CONTEXTS = 200;

function trimContexts(
  contexts: Record<string, MediaServerTaskContext>
): Record<string, MediaServerTaskContext> {
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

function loadStoredContexts(): Record<string, MediaServerTaskContext> {
  try {
    if (typeof localStorage === 'undefined') return {};

    // 1. Check primary storage key
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return trimContexts(parsed);
      }
    }

    // 2. Migration from legacy Emby history contexts if primary key is empty
    const legacyRaw = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacyRaw) {
      const legacyParsed = JSON.parse(legacyRaw);
      if (legacyParsed && typeof legacyParsed === 'object' && !Array.isArray(legacyParsed)) {
        const migrated: Record<string, MediaServerTaskContext> = {};
        for (const [key, ctx] of Object.entries(legacyParsed)) {
          if (ctx && typeof ctx === 'object') {
            const legacyCtx = ctx as Record<string, unknown>;
            migrated[key] = {
              ...(legacyCtx as unknown as MediaServerTaskContext),
              serverId: (legacyCtx.serverId as string) || 'default-emby',
              serverType: (legacyCtx.serverType as 'emby' | 'jellyfin') || 'emby',
            };
          }
        }
        const trimmed = trimContexts(migrated);
        localStorage.setItem(STORAGE_KEY, JSON.stringify(trimmed));
        localStorage.removeItem(LEGACY_STORAGE_KEY);
        return trimmed;
      }
    }

    return {};
  } catch {
    return {};
  }
}

function saveStoredContexts(contexts: Record<string, MediaServerTaskContext>): void {
  try {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(contexts));
  } catch {
    // Ignore storage quota errors
  }
}

interface MediaServerHistoryStore {
  contexts: Record<string, MediaServerTaskContext>;
  setContext: (historyId: string, context: MediaServerTaskContext) => void;
  getContext: (historyId: string) => MediaServerTaskContext | undefined;
  removeContext: (historyId: string) => void;
}

export const useMediaServerHistoryStore = create<MediaServerHistoryStore>((set, get) => ({
  contexts: loadStoredContexts(),

  setContext: (historyId, context) => {
    let trimmedResult: Record<string, MediaServerTaskContext> = {};
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
    let nextContexts: Record<string, MediaServerTaskContext> = {};
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

export const useEmbyHistoryStore = useMediaServerHistoryStore;
