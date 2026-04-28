import { useEffect, useRef } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { TranscriptSegment } from '../types/transcript';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';
import { logger } from '../utils/logger';

const DEFAULT_AUTO_SAVE_DELAY_MS = 2000;
const LIVE_DRAFT_AUTO_SAVE_DELAY_MS = 500;

const autoSaveRuntime = {
    isSaving: false,
    timeout: null as ReturnType<typeof setTimeout> | null,
    lastFingerprint: '',
    pendingHistoryId: null as string | null,
    pendingSegments: null as TranscriptSegment[] | null,
};

async function saveToHistory(historyId: string, segments: TranscriptSegment[]) {
    if (autoSaveRuntime.isSaving) return;

    try {
        autoSaveRuntime.isSaving = true;
        logger.info('[AutoSave] Saving transcript...', historyId);

        await useHistoryStore.getState().updateTranscript(historyId, segments);
        useTranscriptStore.getState().setAutoSaveState(historyId, 'saved');

    } catch (err) {
        logger.error('[AutoSave] Failed to save:', err);
        useTranscriptStore.getState().setAutoSaveState(historyId, 'error');
    } finally {
        autoSaveRuntime.isSaving = false;
    }
}

function queueSave(historyId: string, segments: TranscriptSegment[], delayMs: number) {
    if (autoSaveRuntime.timeout) {
        clearTimeout(autoSaveRuntime.timeout);
    }

    autoSaveRuntime.pendingHistoryId = historyId;
    autoSaveRuntime.pendingSegments = [...segments];
    useTranscriptStore.getState().setAutoSaveState(historyId, 'saving');
    autoSaveRuntime.timeout = setTimeout(() => {
        autoSaveRuntime.timeout = null;
        const queuedHistoryId = autoSaveRuntime.pendingHistoryId;
        const queuedSegments = autoSaveRuntime.pendingSegments;
        autoSaveRuntime.pendingHistoryId = null;
        autoSaveRuntime.pendingSegments = null;
        if (!queuedHistoryId || !queuedSegments) {
            return;
        }
        logger.info('[AutoSave] Debounce triggered for:', queuedHistoryId);
        void saveToHistory(queuedHistoryId, queuedSegments);
    }, delayMs);
}

export async function flushPendingAutoSave(
    historyId?: string | null,
    segments?: TranscriptSegment[] | null,
): Promise<void> {
    const targetHistoryId = historyId || autoSaveRuntime.pendingHistoryId;
    const targetSegments = segments ? [...segments] : autoSaveRuntime.pendingSegments;

    if (!targetHistoryId || !targetSegments) {
        return;
    }

    if (autoSaveRuntime.timeout) {
        clearTimeout(autoSaveRuntime.timeout);
        autoSaveRuntime.timeout = null;
    }

    if (autoSaveRuntime.pendingHistoryId === targetHistoryId) {
        autoSaveRuntime.pendingHistoryId = null;
        autoSaveRuntime.pendingSegments = null;
    }

    useTranscriptStore.getState().setAutoSaveState(targetHistoryId, 'saving');
    await saveToHistory(targetHistoryId, targetSegments);
}

/**
 * Hook to auto-save transcript changes to the history file.
 *
 * - Debounces saves by 2 seconds
 * - Flushes pending saves immediately when switching history items
 * - Updates in-memory history list metadata (preview text)
 */
export function useAutoSaveTranscript() {
    const lastFingerprintRef = useRef<string>('');
    const updateTranscript = useHistoryStore(state => state.updateTranscript);

    // Subscribe to store changes
    useEffect(() => {
        // Initialize fingerprint with current state
        lastFingerprintRef.current = computeSegmentsFingerprint(useTranscriptStore.getState().segments);
        autoSaveRuntime.lastFingerprint = lastFingerprintRef.current;

        const unsubscribe = useTranscriptStore.subscribe(
            (state, prevState) => {
                const currentId = state.sourceHistoryId;
                const prevId = prevState.sourceHistoryId;

                // 1. Handle Switching History Items (Flush pending save for previous item)
                if (prevId && prevId !== currentId) {
                    if (autoSaveRuntime.timeout) {
                        logger.info('[AutoSave] Switching items, flushing save for:', prevId);
                        void flushPendingAutoSave(prevId, prevState.segments);
                    }

                    // Reset fingerprint for new item
                    lastFingerprintRef.current = computeSegmentsFingerprint(state.segments);
                    autoSaveRuntime.lastFingerprint = lastFingerprintRef.current;
                    return;
                }

                // 2. Handle Edit in Current Item
                if (currentId) {
                    // Optimization: Only compute fingerprint if the segments array reference changed
                    if (state.segments !== prevState.segments) {
                        const currentFingerprint = computeSegmentsFingerprint(state.segments);

                        if (currentFingerprint !== lastFingerprintRef.current) {
                            // Change detected
                            lastFingerprintRef.current = currentFingerprint;
                            autoSaveRuntime.lastFingerprint = currentFingerprint;
                            const delayMs = state.mode === 'live'
                                ? LIVE_DRAFT_AUTO_SAVE_DELAY_MS
                                : DEFAULT_AUTO_SAVE_DELAY_MS;
                            queueSave(currentId, state.segments, delayMs);
                        }
                    }
                }
            }
        );


        return () => {
            unsubscribe();
            if (autoSaveRuntime.timeout) {
                clearTimeout(autoSaveRuntime.timeout);
                autoSaveRuntime.timeout = null;
            }
        };
    }, [updateTranscript]);
}
