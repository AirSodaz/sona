import { create } from 'zustand';
import { HistoryItem } from '../types/history';
import { historyService } from '../services/historyService';

interface HistoryState {
    items: HistoryItem[];
    isLoading: boolean;
    error: string | null;

    // Actions
    loadItems: () => Promise<void>;
    addItem: (item: HistoryItem) => void;
    deleteItem: (id: string) => Promise<void>;
    refresh: () => Promise<void>;
}

export const useHistoryStore = create<HistoryState>((set, get) => ({
    items: [],
    isLoading: false,
    error: null,

    loadItems: async () => {
        set({ isLoading: true, error: null });
        try {
            const items = await historyService.getAll();
            set({ items: items || [] }); // Ensure array
        } catch (err: any) {
            console.error('Failed to load history items:', err);
            set({ error: err.message || 'Failed to load history' });
        } finally {
            set({ isLoading: false });
        }
    },

    addItem: (item) => {
        set((state) => ({
            items: [item, ...state.items]
        }));
    },

    deleteItem: async (id) => {
        // Optimistic update
        const originalItems = get().items;
        set((state) => ({
            items: state.items.filter((i) => i.id !== id)
        }));

        try {
            await historyService.deleteRecording(id);
        } catch (err: any) {
            console.error('Failed to delete history item:', err);
            // Revert
            set({ items: originalItems, error: 'Failed to delete item' });
        }
    },

    refresh: async () => {
        await get().loadItems();
    }
}));
