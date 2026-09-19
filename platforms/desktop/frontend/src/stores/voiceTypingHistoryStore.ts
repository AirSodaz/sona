import { create } from 'zustand';

export interface VoiceTypingHistoryItem {
  id: string;
  timestamp: number;
  rawText: string;
  polishedText?: string;
  injectedText: string;
  mode: 'raw' | 'polish';
}

const STORAGE_KEY = 'sona:voice-typing:history';
const MAX_HISTORY_ITEMS = 100;

function loadStoredHistory(): VoiceTypingHistoryItem[] {
  try {
    if (typeof localStorage === 'undefined') return [];
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveHistory(items: VoiceTypingHistoryItem[]): void {
  try {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items.slice(0, MAX_HISTORY_ITEMS)));
  } catch {
    // Ignore storage quota errors
  }
}

interface VoiceTypingHistoryStore {
  items: VoiceTypingHistoryItem[];
  addItem: (item: Omit<VoiceTypingHistoryItem, 'id' | 'timestamp'>) => VoiceTypingHistoryItem;
  removeItem: (id: string) => void;
  clearHistory: () => void;
  reloadHistory: () => void;
}
export const useVoiceTypingHistoryStore = create<VoiceTypingHistoryStore>((set) => ({
  items: loadStoredHistory(),

  addItem: (entry) => {
    const newItem: VoiceTypingHistoryItem = {
      ...entry,
      id: `vt_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      timestamp: Date.now(),
    };

    set((state) => {
      const nextItems = [newItem, ...state.items].slice(0, MAX_HISTORY_ITEMS);
      saveHistory(nextItems);
      return { items: nextItems };
    });

    return newItem;
  },

  removeItem: (id) =>
    set((state) => {
      const nextItems = state.items.filter((item) => item.id !== id);
      saveHistory(nextItems);
      return { items: nextItems };
    }),

  clearHistory: () =>
    set(() => {
      saveHistory([]);
      return { items: [] };
    }),

  reloadHistory: () =>
    set(() => ({
      items: loadStoredHistory(),
    })),
}));

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (event) => {
    if (event.key === STORAGE_KEY) {
      useVoiceTypingHistoryStore.setState({ items: loadStoredHistory() });
    }
  });
}
