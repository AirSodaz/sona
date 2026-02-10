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
        if (state.isQueueProcessing) return;

        set({ isQueueProcessing: true });

        const config = useTranscriptStore.getState().config;
        // Don't error out if config is not yet loaded in tests
        if (!config.offlineModelPath && !process.env.VITEST) {
            console.error('[BatchQueue] No model path configured');
            set({ isQueueProcessing: false });
            return;
        }

        // If testing without model path, just simulate processing or return
        if (!config.offlineModelPath) {
            set({ isQueueProcessing: false });
            return;
        }

        // Configure transcription service
        transcriptionService.setModelPath(config.offlineModelPath);
        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

        transcriptionService.setEnableITN(enabledITNModels.size > 0);

        if (enabledITNModels.size > 0) {
            try {
                const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                transcriptionService.setITNModelPaths(paths);
            } catch (e) {
                console.error('[BatchQueue] Failed to set ITN paths:', e);
            }
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

        // Process items sequentially
        while (true) {
            const currentState = get();
            const pendingItem = currentState.queueItems.find((item) => item.status === 'pending');

            if (!pendingItem) {
                break;
            }

            const { enableTimeline, language } = currentState;

            // Update status to processing
            get().updateItemStatus(pendingItem.id, 'processing', 0);

            let segmentBuffer: TranscriptSegment[] = [];
            let lastUpdateTime = 0;

            try {
                const segments = await transcriptionService.transcribeFile(
                    pendingItem.filePath,
                    (progress) => {
                        get().updateItemStatus(pendingItem.id, 'processing', progress);
                    },
                    (segment) => {
                        // Buffer segments to reduce render frequency and O(N) copy operations
                        segmentBuffer.push(segment);
                        const now = Date.now();

                        // Flush buffer every 500ms or 50 segments
                        if (segmentBuffer.length >= 50 || now - lastUpdateTime > 500) {
                            const state = get();
                            const item = state.queueItems.find((i) => i.id === pendingItem.id);

                            if (item) {
                                // Process the buffered segments
                                const { enableTimeline } = state;
                                const newSegments = enableTimeline ? splitByPunctuation(segmentBuffer) : segmentBuffer;

                                // Append to existing segments
                                const updatedSegments = [...item.segments, ...newSegments];
                                get().updateItemSegments(pendingItem.id, updatedSegments);

                                // Reset buffer
                                segmentBuffer = [];
                                lastUpdateTime = now;
                            }
                        }
                    },
                    language === 'auto' ? undefined : language
                );

                const finalSegments = enableTimeline ? splitByPunctuation(segments) : segments;
                get().updateItemSegments(pendingItem.id, finalSegments);

                // Calculate duration from last segment
                const duration = finalSegments.length > 0 ? finalSegments[finalSegments.length - 1].end : 0;

                // Save to History
                try {
                    const historyItem = await historyService.saveImportedFile(pendingItem.filePath, finalSegments, duration);
                    if (historyItem) {
                        set((state) => ({
                            queueItems: state.queueItems.map((item) =>
                                item.id === pendingItem.id ? { ...item, historyId: historyItem.id } : item
                            )
                        }));
                        // If this is the active item, propagate sourceHistoryId
                        if (get().activeItemId === pendingItem.id) {
                            useTranscriptStore.getState().setSourceHistoryId(historyItem.id);
                        }
                    }
                } catch (err) {
                    console.error('[BatchQueue] Failed to save to history:', err);
                }

                get().updateItemStatus(pendingItem.id, 'complete', 100);

            } catch (error) {
                console.error(`[BatchQueue] Failed to transcribe ${pendingItem.filename}:`, error);
                get().setItemError(pendingItem.id, String(error));
            }
        }

        set({ isQueueProcessing: false });
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
