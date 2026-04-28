import { create } from 'zustand';
import { useAutomationStore } from './automationStore';
import { useBatchQueueStore } from './batchQueueStore';
import {
    loadRecoverySnapshot,
    markAutomationRecoveryItemsResumed,
    saveRecoveredItems,
} from '../services/recoveryService';
import type { RecoveredQueueItem } from '../types/recovery';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';

interface RecoveryState {
    items: RecoveredQueueItem[];
    updatedAt: number | null;
    isLoaded: boolean;
    isBusy: boolean;
    error: string | null;
    loadRecovery: () => Promise<void>;
    resumeAll: () => Promise<void>;
    discardItem: (id: string) => Promise<void>;
    discardAll: () => Promise<void>;
    clearResolved: () => Promise<void>;
}

async function persistRecoveryItems(items: RecoveredQueueItem[]): Promise<void> {
    await saveRecoveredItems(items);
}

export const useRecoveryStore = create<RecoveryState>((set, get) => ({
    items: [],
    updatedAt: null,
    isLoaded: false,
    isBusy: false,
    error: null,

    loadRecovery: async () => {
        set({ isBusy: true, error: null });

        try {
            const snapshot = await loadRecoverySnapshot();
            set({
                items: snapshot.items,
                updatedAt: snapshot.updatedAt,
                isLoaded: true,
                isBusy: false,
                error: null,
            });
        } catch (error) {
            const errorMessage = extractErrorMessage(error) || 'Failed to load recovery state.';
            logger.error('[Recovery] Failed to load recovery state:', error);
            set({
                items: [],
                updatedAt: null,
                isLoaded: true,
                isBusy: false,
                error: errorMessage,
            });
        }
    },

    resumeAll: async () => {
        const pendingItems = get().items.filter((item) => item.resolution === 'pending');
        if (pendingItems.length === 0) {
            return;
        }

        if (pendingItems.some((item) => !item.canResume)) {
            throw new Error('Discard missing source files before resuming recovery.');
        }

        set({ isBusy: true, error: null });

        try {
            markAutomationRecoveryItemsResumed(pendingItems);
            useBatchQueueStore.getState().enqueueRecoveredItems(pendingItems);
            set({
                items: [],
                updatedAt: null,
                isBusy: false,
                error: null,
            });
        } catch (error) {
            const errorMessage = extractErrorMessage(error) || 'Failed to resume recovery items.';
            logger.error('[Recovery] Failed to resume recovery items:', error);
            set({
                isBusy: false,
                error: errorMessage,
            });
            throw error;
        }
    },

    discardItem: async (id) => {
        const target = get().items.find((item) => item.id === id);
        if (!target) {
            return;
        }

        set({ isBusy: true, error: null });

        try {
            if (target.source === 'automation') {
                await useAutomationStore.getState().markRecoveryItemDiscarded(target);
            }

            const nextItems = get().items.filter((item) => item.id !== id);
            await persistRecoveryItems(nextItems);
            set({
                items: nextItems,
                updatedAt: nextItems.length > 0 ? Date.now() : null,
                isBusy: false,
                error: null,
            });
        } catch (error) {
            const errorMessage = extractErrorMessage(error) || 'Failed to discard recovery item.';
            logger.error('[Recovery] Failed to discard recovery item:', error);
            set({
                isBusy: false,
                error: errorMessage,
            });
            throw error;
        }
    },

    discardAll: async () => {
        const items = get().items;
        if (items.length === 0) {
            return;
        }

        set({ isBusy: true, error: null });

        try {
            await Promise.all(items
                .filter((item) => item.source === 'automation')
                .map((item) => useAutomationStore.getState().markRecoveryItemDiscarded(item)));
            await persistRecoveryItems([]);
            set({
                items: [],
                updatedAt: null,
                isBusy: false,
                error: null,
            });
        } catch (error) {
            const errorMessage = extractErrorMessage(error) || 'Failed to discard recovery items.';
            logger.error('[Recovery] Failed to discard all recovery items:', error);
            set({
                isBusy: false,
                error: errorMessage,
            });
            throw error;
        }
    },

    clearResolved: async () => {
        const nextItems = get().items.filter((item) => item.resolution === 'pending');
        await persistRecoveryItems(nextItems);
        set({
            items: nextItems,
            updatedAt: nextItems.length > 0 ? Date.now() : null,
            error: null,
        });
    },
}));
