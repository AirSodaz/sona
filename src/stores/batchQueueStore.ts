import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { convertFileSrc } from '@tauri-apps/api/core';
import { BatchQueueItem, BatchQueueItemStatus } from '../types/batchQueue';
import { TranscriptSegment } from '../types/transcript';
import { transcriptionService } from '../services/transcriptionService';
import { historyService } from '../services/historyService';
import { modelService } from '../services/modelService';
import { useTranscriptStore } from './transcriptStore';
import { splitByPunctuation } from '../utils/segmentUtils';
import { tempDir, join } from '@tauri-apps/api/path';
import { remove } from '@tauri-apps/plugin-fs';

/** State interface for the batch queue store. */
interface BatchQueueState {
    /** List of queued files. */
    queueItems: BatchQueueItem[];
    /** ID of the currently active/selected item. */
    activeItemId: string | null;
    /** Whether the queue is currently processing. */
    isQueueProcessing: boolean;
    /** Whether to enable timeline mode (split by punctuation). */
    enableTimeline: boolean;
    /** Language setting for transcription. */
    language: string;

    /**
     * Adds files to the queue.
     *
     * @param filePaths Array of file paths to add.
     */
    addFiles: (filePaths: string[]) => void;

    /**
     * Starts processing the queue sequentially.
     */
    processQueue: () => Promise<void>;

    /**
     * Sets the active/selected item.
     *
     * @param id Item ID to set as active.
     */
    setActiveItem: (id: string | null) => void;

    /**
     * Updates an item's status and progress.
     *
     * @param id Item ID.
     * @param status New status.
     * @param progress New progress value.
     */
    updateItemStatus: (id: string, status: BatchQueueItemStatus, progress?: number) => void;

    /**
     * Updates an item's segments.
     *
     * @param id Item ID.
     * @param segments New segments array.
     */
    updateItemSegments: (id: string, segments: TranscriptSegment[]) => void;

    /**
     * Sets error state for an item.
     *
     * @param id Item ID.
     * @param message Error message.
     */
    setItemError: (id: string, message: string) => void;

    /**
     * Removes an item from the queue.
     *
     * @param id Item ID to remove.
     */
    removeItem: (id: string) => void;

    /** Clears all items from the queue. */
    clearQueue: () => void;

    /**
     * Sets the timeline mode setting.
     *
     * @param enabled Whether to enable timeline mode.
     */
    setEnableTimeline: (enabled: boolean) => void;

    /**
     * Sets the language setting.
     *
     * @param language Language code.
     */
    setLanguage: (language: string) => void;
    /** internal helper */
    _processItem: (itemId: string) => Promise<void>;
}

/**
 * Zustand store for managing batch transcription queue.
 */
export const useBatchQueueStore = create<BatchQueueState>((set, get) => ({
    queueItems: [],
    activeItemId: null,
    isQueueProcessing: false,
    enableTimeline: true,
    language: 'auto',

    addFiles: (filePaths) => {
        const newItems: BatchQueueItem[] = filePaths.map((filePath) => {
            const filename = filePath.split(/[/\\]/).pop() || filePath;
            return {
                id: uuidv4(),
                filename,
                filePath,
                status: 'pending',
                progress: 0,
                segments: [],
                audioUrl: convertFileSrc(filePath),
            };
        });

        set((state) => ({
            queueItems: [...state.queueItems, ...newItems]
        }));

        // If no active item, set the first new item as active
        const state = get();
        if (!state.activeItemId && newItems.length > 0) {
            get().setActiveItem(newItems[0].id);
        }

        // Auto-start processing if not already running
        if (!state.isQueueProcessing) {
            get().processQueue();
        }
    },


    processQueue: async () => {
        const state = get();
        // Derived from transcript store (default 2)
        const maxConcurrent = useTranscriptStore.getState().config.maxConcurrent || 2;

        // Count how many are currently processing
        const processingCount = state.queueItems.filter(i => i.status === 'processing').length;

        // If we are already at capacity, do nothing
        if (processingCount >= maxConcurrent) {
            return;
        }

        const config = useTranscriptStore.getState().config;
        // Don't error out if config is not yet loaded in tests
        if (!config.offlineModelPath && !process.env.VITEST) {
            console.error('[BatchQueue] No model path configured');
            return;
        }

        // If testing without model path, just simulate processing or return
        if (!config.offlineModelPath) {
            return;
        }

        // Configure transcription service (safe to call multiple times)
        transcriptionService.setModelPath(config.offlineModelPath);
        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

        transcriptionService.setEnableITN(config.enableITN ?? false);

        if (enabledITNModels.size > 0) {
            try {
                const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                transcriptionService.setITNModelPaths(paths);
            } catch (e) {
                console.error('[BatchQueue] Failed to set ITN paths:', e);
                transcriptionService.setITNModelPaths([]);
            }
        } else {
            transcriptionService.setITNModelPaths([]);
        }

        if (config.punctuationModelPath) {
            transcriptionService.setPunctuationModelPath(config.punctuationModelPath);
        } else {
            transcriptionService.setPunctuationModelPath('');
        }

        if (config.vadModelPath) {
            transcriptionService.setVadModelPath(config.vadModelPath);
            transcriptionService.setVadBufferSize(config.vadBufferSize || 5);
        }

        transcriptionService.setCtcModelPath(config.ctcModelPath || '');

        // Find items to start
        const pendingItems = state.queueItems.filter(item => item.status === 'pending');
        const slotsAvailable = maxConcurrent - processingCount;
        const itemsToStart = pendingItems.slice(0, slotsAvailable);

        if (itemsToStart.length === 0 && processingCount === 0) {
            set({ isQueueProcessing: false });
            return;
        }

        set({ isQueueProcessing: true });

        // Start processing for each new item
        itemsToStart.forEach(item => {
            get()._processItem(item.id);
        });
    },

    /**
     * Internal helper to process a single item.
     * @param itemId ID of the item to process.
     */
    _processItem: async (itemId: string) => {
        const state = get();
        const item = state.queueItems.find(i => i.id === itemId);
        if (!item || item.status !== 'pending') return;

        const { enableTimeline, language } = state;

        // Update status to processing
        get().updateItemStatus(itemId, 'processing', 0);

        let segmentBuffer: TranscriptSegment[] = [];
        let lastUpdateTime = 0;
        let tempWavPath: string | undefined;

        try {
            const tempD = await tempDir();
            tempWavPath = await join(tempD, `${uuidv4()}.wav`);

            const segments = await transcriptionService.transcribeFile(
                item.filePath,
                (progress) => {
                    get().updateItemStatus(itemId, 'processing', progress);
                },
                (segment) => {
                    // Buffer segments to reduce render frequency and O(N) copy operations
                    segmentBuffer.push(segment);
                    const now = Date.now();

                    // Flush buffer every 500ms or 50 segments
                    if (segmentBuffer.length >= 50 || now - lastUpdateTime > 500) {
                        const currentState = get(); // Re-fetch state
                        const currentItem = currentState.queueItems.find((i) => i.id === itemId);

                        if (currentItem) {
                            // Process the buffered segments
                            const { enableTimeline } = currentState;
                            const newSegments = enableTimeline ? splitByPunctuation(segmentBuffer) : segmentBuffer;

                            // Append to existing segments
                            const updatedSegments = [...currentItem.segments, ...newSegments];
                            get().updateItemSegments(itemId, updatedSegments);

                            // Reset buffer
                            segmentBuffer = [];
                            lastUpdateTime = now;
                        }
                    }
                },
                language === 'auto' ? undefined : language,
                tempWavPath
            );

            const finalSegments = enableTimeline ? splitByPunctuation(segments) : segments;
            get().updateItemSegments(itemId, finalSegments);

            // Calculate duration from last segment
            const duration = finalSegments.length > 0 ? finalSegments[finalSegments.length - 1].end : 0;

            // Save to History
            try {
                const historyItem = await historyService.saveImportedFile(item.filePath, finalSegments, duration, tempWavPath);
                if (historyItem) {
                    set((state) => ({
                        queueItems: state.queueItems.map((i) =>
                            i.id === itemId ? { ...i, historyId: historyItem.id } : i
                        )
                    }));
                    // If this is the active item, propagate sourceHistoryId
                    if (get().activeItemId === itemId) {
                        useTranscriptStore.getState().setSourceHistoryId(historyItem.id);
                    }
                }
            } catch (err) {
                console.error('[BatchQueue] Failed to save to history:', err);
            }

            // Cleanup temp file
            if (tempWavPath) {
                try {
                    await remove(tempWavPath);
                } catch (e) {
                    console.warn('[BatchQueue] Failed to remove temp file:', e);
                }
            }

            get().updateItemStatus(itemId, 'complete', 100);

        } catch (error) {
            console.error(`[BatchQueue] Failed to transcribe ${item.filename}:`, error);
            get().setItemError(itemId, String(error));
        } finally {
            // Trigger next items in queue
            get().processQueue();
        }
    },


    setActiveItem: (id) => {
        set({ activeItemId: id });

        // Update the main transcript store with the active item's data
        const state = get();
        const item = state.queueItems.find((i) => i.id === id);
        if (item) {
            // Use atomic load to prevent auto-save from overwriting previous item
            useTranscriptStore.getState().loadTranscript(item.segments, item.historyId || null);
            useTranscriptStore.getState().setAudioUrl(item.audioUrl || null);
            // Set source file path for CTC alignment
            transcriptionService.setSourceFilePath(item.filePath);
        } else if (id === null) {
            // Clear if null
            useTranscriptStore.getState().loadTranscript([], null);
            useTranscriptStore.getState().setAudioUrl(null);
            transcriptionService.setSourceFilePath('');
        }
    },

    updateItemStatus: (id, status, progress) => {
        set((state) => ({
            queueItems: state.queueItems.map((item) =>
                item.id === id
                    ? { ...item, status, progress: progress !== undefined ? progress : item.progress }
                    : item
            ),
        }));
    },

    updateItemSegments: (id, segments) => {
        set((state) => ({
            queueItems: state.queueItems.map((item) =>
                item.id === id ? { ...item, segments } : item
            ),
        }));

        // If this is the active item, also update the main store
        const state = get();
        if (state.activeItemId === id) {
            useTranscriptStore.getState().setSegments(segments);
        }
    },

    setItemError: (id, message) => {
        set((state) => ({
            queueItems: state.queueItems.map((item) =>
                item.id === id
                    ? { ...item, status: 'error', errorMessage: message }
                    : item
            ),
        }));
    },

    removeItem: (id) => {
        const state = get();
        const newItems = state.queueItems.filter((item) => item.id !== id);

        // Calculate active item update before state change
        const isActiveItem = state.activeItemId === id;
        const newActiveId = newItems.length > 0 ? newItems[0].id : null;

        set({ queueItems: newItems });

        // If we removed the active item, select the first remaining item
        if (isActiveItem) {
            get().setActiveItem(newActiveId);
        }
    },

    clearQueue: () => {
        set({
            queueItems: [],
            activeItemId: null,
            isQueueProcessing: false,
        });
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.getState().setAudioUrl(null);
    },

    setEnableTimeline: (enabled) => {
        set({ enableTimeline: enabled });
    },

    setLanguage: (language) => {
        set({ language });
    },
}));

/** Selector for queue items. */
export const useQueueItems = () => useBatchQueueStore((state) => state.queueItems);

/** Selector for active item ID. */
export const useActiveItemId = () => useBatchQueueStore((state) => state.activeItemId);

/** Selector for processing state. */
export const useIsQueueProcessing = () => useBatchQueueStore((state) => state.isQueueProcessing);
