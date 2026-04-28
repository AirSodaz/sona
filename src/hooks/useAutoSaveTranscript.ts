import { useEffect, useRef } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { TranscriptSegment } from '../types/transcript';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';
import { logger } from '../utils/logger';

/**
 * Hook to auto-save transcript changes to the history file.
 *
 * - Debounces saves by 2 seconds
 * - Flushes pending saves immediately when switching history items
 * - Updates in-memory history list metadata (preview text)
 */
export function useAutoSaveTranscript() {
    const isSavingRef = useRef(false);
    const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const lastFingerprintRef = useRef<string>('');
    const updateTranscript = useHistoryStore(state => state.updateTranscript);

    // Subscribe to store changes
    useEffect(() => {
        // Initialize fingerprint with current state
        lastFingerprintRef.current = computeSegmentsFingerprint(useTranscriptStore.getState().segments);

        const unsubscribe = useTranscriptStore.subscribe(
            (state, prevState) => {
                const currentId = state.sourceHistoryId;
                const prevId = prevState.sourceHistoryId;

                // 1. Handle Switching History Items (Flush pending save for previous item)
                if (prevId && prevId !== currentId) {
                    if (timeoutRef.current) {
                        clearTimeout(timeoutRef.current);
                        timeoutRef.current = null;

                        // Flush save for PREVIOUS item using PREVIOUS segments
                        const prevSegments = prevState.segments;
                        useTranscriptStore.getState().setAutoSaveState(prevId, 'saving');
                        logger.info('[AutoSave] Switching items, flushing save for:', prevId);
                        saveToHistory(prevId, prevSegments);
                    }

                    // Reset fingerprint for new item
                    lastFingerprintRef.current = computeSegmentsFingerprint(state.segments);
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

                            // Debounce save
                            if (timeoutRef.current) {
                                clearTimeout(timeoutRef.current);
                            }

                            useTranscriptStore.getState().setAutoSaveState(currentId, 'saving');
                            timeoutRef.current = setTimeout(() => {
                                logger.info('[AutoSave] Debounce triggered for:', currentId);
                                saveToHistory(currentId, state.segments);
                            }, 2000);
                        }
                    }
                }
            }
        );


        return () => {
            unsubscribe();
            if (timeoutRef.current) {
                clearTimeout(timeoutRef.current);
            }
        };
    }, [updateTranscript]);

    async function saveToHistory(historyId: string, segments: TranscriptSegment[]) {
        if (isSavingRef.current) return;

        try {
            isSavingRef.current = true;
            logger.info('[AutoSave] Saving transcript...', historyId);

            await updateTranscript(historyId, segments);
            useTranscriptStore.getState().setAutoSaveState(historyId, 'saved');

        } catch (err) {
            logger.error('[AutoSave] Failed to save:', err);
            useTranscriptStore.getState().setAutoSaveState(historyId, 'error');
        } finally {
            isSavingRef.current = false;
        }
    }
}
