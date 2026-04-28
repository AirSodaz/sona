import { create } from 'zustand';
import { HistoryItem } from '../types/history';
import { historyService } from '../services/historyService';
import { TranscriptSegment } from '../types/transcript';
import { buildHistoryTranscriptMetadata } from '../utils/historyTranscriptMetadata';
import { useTranscriptStore } from './transcriptStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';

interface HistoryState {
    items: HistoryItem[];
    isLoading: boolean;
    error: string | null;

    // Actions
    loadItems: () => Promise<void>;
    addItem: (item: HistoryItem) => void;
    deleteItem: (id: string) => Promise<void>;
    deleteItems: (ids: string[]) => Promise<void>;
    refresh: () => Promise<void>;
    /**
     * Updates a saved transcript and synchronizes derived metadata back into the in-memory history list.
     *
     * @param id The history item ID.
     * @param segments The latest transcript segments.
     * @return A promise that resolves when the update is complete.
     */
    updateTranscript: (id: string, segments: TranscriptSegment[]) => Promise<void>;
    /**
     * Updates metadata fields for a specific history item in the in-memory list and on disk.
     *
     * @param id The history item ID.
     * @param updates Partial fields to merge into the item.
     * @return A promise that resolves when the update is complete.
     */
    updateItemMeta: (id: string, updates: Partial<HistoryItem>) => Promise<void>;
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
        } catch (error) {
            const errorMessage = extractErrorMessage(error);
            logger.error('Failed to load history items:', error);
            set({ error: errorMessage || 'Failed to load history' });
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
            useTranscriptStore.getState().clearSummaryState(id);
            
            // Clear current transcript if it matches the deleted item
            const transcriptStore = useTranscriptStore.getState();
            if (transcriptStore.sourceHistoryId === id) {
                transcriptStore.clearSegments();
                transcriptStore.setAudioUrl(null);
                transcriptStore.setAudioFile(null);
                transcriptStore.setSourceHistoryId(null);
            }
        } catch (error) {
            logger.error('Failed to delete history item:', error);
            // Revert
            set({ items: originalItems, error: 'Failed to delete item' });
        }
    },

    deleteItems: async (ids) => {
        const originalItems = get().items;
        set((state) => ({
            items: state.items.filter((i) => !ids.includes(i.id))
        }));

        try {
            await historyService.deleteRecordings(ids);
            ids.forEach((id) => useTranscriptStore.getState().clearSummaryState(id));
            
            // Clear current transcript if it matches any of the deleted items
            const transcriptStore = useTranscriptStore.getState();
            if (transcriptStore.sourceHistoryId && ids.includes(transcriptStore.sourceHistoryId)) {
                transcriptStore.clearSegments();
                transcriptStore.setAudioUrl(null);
                transcriptStore.setAudioFile(null);
                transcriptStore.setSourceHistoryId(null);
            }
        } catch (error) {
            logger.error('Failed to delete history items:', error);
            set({ items: originalItems, error: 'Failed to delete items' });
        }
    },

    refresh: async () => {
        await get().loadItems();
    },

    updateTranscript: async (id, segments) => {
        try {
            await historyService.updateTranscript(id, segments);
            const transcriptMetadata = buildHistoryTranscriptMetadata(segments);

            set((state) => ({
                items: state.items.map((item) => (
                    item.id === id ? { ...item, ...transcriptMetadata } : item
                )),
            }));
        } catch (error) {
            const errorMessage = extractErrorMessage(error);
            logger.error('Failed to update history transcript:', error);
            set({ error: errorMessage || 'Failed to update history transcript' });
            throw error;
        }
    },

    updateItemMeta: async (id, updates) => {
        const originalItems = get().items;
        
        // Optimistic update
        set((state) => ({
            items: state.items.map((item) =>
                item.id === id ? { ...item, ...updates } : item
            )
        }));

        try {
            await historyService.updateItemMeta(id, updates);
        } catch (error) {
            logger.error('Failed to update history item meta:', error);
            // Revert
            set({ items: originalItems, error: 'Failed to update item metadata' });
            throw error;
        }
    }
}));
